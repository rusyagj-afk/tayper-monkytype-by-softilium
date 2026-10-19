mod browser;
mod cdp;
mod cli;
mod engine;

use clap::Parser;
use cli::{prompt_input, CliArgs, TriggerMode};
use engine::{get_adjacent_typo, Pacer, TimingProfile, TypingMode};
use serde_json::Value;
use std::io::{self, Write};
use std::time::Duration;

const DOM_QUERY_SCRIPT: &str = r#"(() => {
    const result = document.querySelector('#result');
    if (result && !result.classList.contains('hidden') && result.offsetHeight > 0) {
        const query = s => {
            const el = document.querySelector(s);
            return el ? el.textContent.trim() : null;
        };
        return {
            finished: true,
            wpm: query('#result .group.wpm .bottom') || query('#result .wpm .val'),
            acc: query('#result .group.acc .bottom') || query('#result .acc .val'),
            raw: query('#result .group.raw .bottom') || query('#result .raw .val'),
            consistency: query('#result .group.key .bottom') || query('#result .consistency .val'),
        };
    }
    const active = document.querySelector('#words .word.active');
    if (!active) return { finished: false, chars: null };
    const letters = active.querySelectorAll('letter:not(.correct):not(.incorrect)');
    return { finished: false, chars: Array.from(letters).map(l => l.textContent).join('') };
})()"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    println!("\x1b[1;36msoftilium v1.2.0\x1b[0m — Monkeytype Automation Client\n");

    let is_interactive = args.wpm.is_none()
        && args.browser.is_none()
        && args.mode.is_none()
        && args.trigger.is_none();

    let detected = browser::detect_browsers();

    let selected_browser = if let Some(ref name) = args.browser {
        let needle = name.to_lowercase();
        detected
            .iter()
            .find(|b| b.name.to_lowercase().contains(&needle) || b.process_name.contains(&needle))
            .cloned()
            .unwrap_or_else(|| browser::Browser {
                name: name.clone(),
                path: name.clone(),
                process_name: name.clone(),
            })
    } else if is_interactive && !detected.is_empty() {
        println!("\x1b[1;33m[1/3] Select browser:\x1b[0m");
        for (idx, b) in detected.iter().enumerate() {
            println!("  [{}] {} ({})", idx + 1, b.name, b.path);
        }
        println!(
            "  [{}] Connect to running instance on port 9222",
            detected.len() + 1
        );

        let choice = prompt_input("  Choice", "1");
        let idx = choice.parse::<usize>().unwrap_or(1);

        if idx >= 1 && idx <= detected.len() {
            detected[idx - 1].clone()
        } else {
            browser::Browser {
                name: "Existing session".to_string(),
                path: String::new(),
                process_name: String::new(),
            }
        }
    } else if !detected.is_empty() {
        detected[0].clone()
    } else {
        browser::Browser {
            name: "Default".to_string(),
            path: "helium".to_string(),
            process_name: "helium".to_string(),
        }
    };

    let target_wpm = if let Some(w) = args.wpm {
        w.max(10.0)
    } else if is_interactive {
        println!("\n\x1b[1;33m[2/3] Target speed (WPM):\x1b[0m");
        println!("  [1] 100 WPM");
        println!("  [2] 150 WPM");
        println!("  [3] 180 WPM");
        println!("  [4] 200 WPM");
        println!("  [5] 250 WPM");
        println!("  [6] Custom input");

        let choice = prompt_input("  Choice", "4");
        match choice.as_str() {
            "1" => 100.0,
            "2" => 150.0,
            "3" => 180.0,
            "4" => 200.0,
            "5" => 250.0,
            "6" => {
                let custom = prompt_input("  Enter WPM", "200");
                custom.parse::<f64>().unwrap_or(200.0).max(10.0)
            }
            raw => raw.parse::<f64>().unwrap_or(200.0).max(10.0),
        }
    } else {
        200.0
    };

    let typing_mode = if let Some(ref m) = args.mode {
        match m.to_lowercase().as_str() {
            "flat" | "robot" => TypingMode::Flat,
            "smooth" | "100" => TypingMode::Smooth,
            _ => TypingMode::Legit,
        }
    } else if is_interactive {
        println!("\n\x1b[1;33m[3/3] Humanization profile:\x1b[0m");
        println!("  [1] Legit Pro (wave velocity, cognitive pauses, typos ~98% acc) [Recommended]");
        println!("  [2] Legit Smooth (wave velocity, micro-pauses, 100% accuracy)");
        println!("  [3] Flat (linear robotic pacing)");

        let choice = prompt_input("  Choice", "1");
        match choice.as_str() {
            "2" => TypingMode::Smooth,
            "3" => TypingMode::Flat,
            _ => TypingMode::Legit,
        }
    } else {
        TypingMode::Legit
    };

    let trigger_mode = if let Some(ref t) = args.trigger {
        match t.to_lowercase().as_str() {
            "manual" | "key" => TriggerMode::FirstKey,
            "delay" | "timer" => TriggerMode::Delay,
            "direct" | "countdown" => TriggerMode::Direct,
            _ => TriggerMode::Hotkey,
        }
    } else {
        TriggerMode::Hotkey
    };

    if !selected_browser.path.is_empty() && !browser::ping_cdp(&args.cdp).await {
        let running = browser::is_running(&selected_browser.process_name);
        let isolate = if running {
            println!(
                "\nBrowser {} is already running without debugging flags.",
                selected_browser.name
            );
            let choice = prompt_input("Restart with CDP [1] or launch isolated profile [2]", "1");
            if choice == "1" {
                browser::kill(&selected_browser.process_name);
                tokio::time::sleep(Duration::from_secs(1)).await;
                false
            } else {
                true
            }
        } else {
            false
        };

        browser::launch_instance(&selected_browser, &args.cdp, isolate).await?;
    }

    println!("Locating target tab...");
    let tab = browser::fetch_active_tab(&args.cdp).await?;
    println!("Attached to: \"{}\"", tab.title);

    let ws_url = tab
        .web_socket_debugger_url
        .ok_or("Debugger URL missing on target page")?;

    let cdp = cdp::CdpClient::connect(&ws_url).await?;

    match trigger_mode {
        TriggerMode::Hotkey => {
            cdp.eval(
                r#"(() => {
                window.__softilium_hotkey = false;
                window.addEventListener('keydown', (e) => {
                    if (e.key === 'F8' || e.code === 'F8') {
                        e.preventDefault();
                        window.__softilium_hotkey = true;
                    }
                }, { once: true });
            })()"#,
            )
            .await?;

            println!(
                "\n\x1b[1;35m[Stealth Hotkey: F8]\x1b[0m Focus the browser and press F8 to begin."
            );
            loop {
                tokio::time::sleep(Duration::from_millis(40)).await;
                let active = cdp
                    .eval("window.__softilium_hotkey === true")
                    .await
                    .unwrap_or(Value::Bool(false));
                if active == Value::Bool(true) {
                    break;
                }
            }
        }
        TriggerMode::FirstKey => {
            cdp.eval("document.querySelector('#wordsInput')?.focus();")
                .await?;
            println!(
                "\n\x1b[1;35m[Manual Trigger]\x1b[0m Type the first character manually in browser."
            );
            loop {
                tokio::time::sleep(Duration::from_millis(30)).await;
                let has_typed = cdp
                    .eval(
                        r#"(() => {
                    const active = document.querySelector('#words .word.active');
                    if (!active) return false;
                    return active.querySelectorAll('letter.correct, letter.incorrect').length > 0;
                })()"#,
                    )
                    .await
                    .unwrap_or(Value::Bool(false));

                if has_typed == Value::Bool(true) {
                    break;
                }
            }
        }
        TriggerMode::Delay => {
            println!("\nDelay trigger: starting in 8 seconds...");
            tokio::time::sleep(Duration::from_secs(8)).await;
        }
        TriggerMode::Direct => {
            cdp.eval("document.querySelector('#wordsInput')?.focus();")
                .await?;
            for i in (1..=3).rev() {
                println!("{i}...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    cdp.eval("document.querySelector('#wordsInput')?.focus();")
        .await?;

    let timing = TimingProfile::new(target_wpm, typing_mode);
    let mut pacer = Pacer::new();

    let mut typed_keystrokes: usize = 0;
    let mut words_completed: usize = 0;
    let mut empty_streak: usize = 0;
    let mut last_char: Option<char> = None;

    loop {
        let state = cdp.eval(DOM_QUERY_SCRIPT).await?;

        if state.get("finished").and_then(|v| v.as_bool()) == Some(true) {
            println!("\n\nTest completed.");
            let extract = |k: &str| {
                state
                    .get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or("—")
                    .to_string()
            };

            println!("Results:");
            println!("  WPM:         {}", extract("wpm"));
            println!("  Accuracy:    {}", extract("acc"));
            println!("  Raw WPM:     {}", extract("raw"));
            println!("  Consistency: {}", extract("consistency"));

            let secs = pacer.elapsed().as_secs_f64();
            if secs > 0.0 {
                let actual_wpm = ((typed_keystrokes as f64) / 5.0) / (secs / 60.0);
                println!("Session Metrics:");
                println!("  Keystrokes:  {typed_keystrokes}");
                println!("  Words:       {words_completed}");
                println!("  Elapsed:     {secs:.2}s");
                println!("  Output WPM:  {actual_wpm:.1} (target: {target_wpm})");
            }
            break;
        }

        let remaining = match state.get("chars").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                empty_streak += 1;
                if empty_streak > 40 {
                    let active = cdp
                        .eval("document.querySelector('#words .word.active') !== null")
                        .await
                        .unwrap_or(Value::Bool(false));
                    if active == Value::Bool(false) {
                        break;
                    }
                    empty_streak = 0;
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
                continue;
            }
        };

        empty_streak = 0;

        if timing.mode != TypingMode::Flat && remaining.len() > 7 {
            tokio::time::sleep(Duration::from_millis(40)).await;
        }

        for (idx, ch) in remaining.chars().enumerate() {
            pacer.start();

            let (char_delay, dwell_ms) =
                timing.calculate_keystroke_delay(pacer.elapsed(), idx, last_char, ch);

            pacer.pace(char_delay).await;

            if timing.should_generate_typo(idx) {
                let wrong = get_adjacent_typo(ch);
                cdp.dispatch_char(wrong, dwell_ms).await?;
                typed_keystrokes += 1;

                tokio::time::sleep(Duration::from_millis(110)).await;
                cdp.dispatch_backspace(dwell_ms).await?;
                typed_keystrokes += 1;

                tokio::time::sleep(Duration::from_millis(45)).await;
            }

            cdp.dispatch_char(ch, dwell_ms).await?;
            typed_keystrokes += 1;
            last_char = Some(ch);
        }

        let (space_delay, space_dwell) = timing.calculate_space_delay();
        pacer.pace(space_delay).await;

        cdp.dispatch_space(space_dwell).await?;
        typed_keystrokes += 1;
        words_completed += 1;
        last_char = Some(' ');

        let mins = pacer.elapsed().as_secs_f64() / 60.0;
        let cur = if mins > 0.0 {
            ((typed_keystrokes as f64) / 5.0) / mins
        } else {
            target_wpm
        };

        print!("\rWords: {words_completed:<4} | Keystrokes: {typed_keystrokes:<5} | Output: {cur:<5.1} WPM");
        let _ = io::stdout().flush();
    }

    Ok(())
}
