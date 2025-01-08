use std::io;
use std::io::Write;

// From https://docs.rs/clt/latest/src/clt/term.rs.html#277-293

fn get_prompt_input(prompt_text: &str) -> String {
    print!("{}", prompt_text);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    input.trim_end_matches('\n').to_string()
}

pub fn confirm(text: &str, default: bool, prompt_suffix: &str, show_default: bool) -> bool {
    let default_string = match default {
        true => Some("Y/n"),
        false => Some("y/N"),
    };
    let prompt_text = build_prompt_text(text, prompt_suffix, show_default, default_string);

    loop {
        let prompt_input = get_prompt_input(&prompt_text).to_ascii_lowercase();
        match prompt_input.trim() {
            "y" | "yes" => {
                return true;
            }
            "n" | "no" => {
                return false;
            }
            "" => {
                return default;
            }
            _ => {
                println!("Error: invalid input");
            }
        }
    }
}

fn build_prompt_text(
    text: &str,
    suffix: &str,
    show_default: bool,
    default: Option<&str>,
) -> String {
    let prompt_text = if default.is_some() && show_default {
        format!("{} [{}]", text, default.unwrap())
    } else {
        text.to_string()
    };
    prompt_text + suffix
}
