pub const ASCII_ART: &str = r"
   █████████      ██████████      ███████████    ███████████ 
  ███░░░░░███    ░░███░░░░███    ░█░░░███░░░█   ░░███░░░░░███
 ░███    ░███     ░███   ░░███   ░   ░███  ░     ░███    ░███
 ░███████████     ░███    ░███       ░███        ░██████████ 
 ░███░░░░░███     ░███    ░███       ░███        ░███░░░░░░  
 ░███    ░███     ░███    ███        ░███        ░███        
 █████   █████ ██ ██████████   ██    █████    ██ █████       
░░░░░   ░░░░░ ░░ ░░░░░░░░░░   ░░    ░░░░░    ░░ ░░░░░       
";

pub const VERSION: &str = "v0.1.0";

pub fn print_art() {
    let gold = if supports_color::on_cached(supports_color::Stream::Stdout).map(|c| c.has_16m).unwrap_or(false) {
        "\x1b[38;2;201;168;76m"
    } else {
        "\x1b[38;5;214m"
    };
    let reset = "\x1b[0m";
    println!("{gold}{ASCII_ART}{reset}");
}

pub fn gold_style() -> &'static str {
    if supports_color::on_cached(supports_color::Stream::Stdout).map(|c| c.has_16m).unwrap_or(false) {
        "\x1b[38;2;201;168;76m"
    } else {
        "\x1b[38;5;214m"
    }
}

pub fn reset_style() -> &'static str {
    "\x1b[0m"
}

pub fn dim() -> &'static str {
    "\x1b[2m"
}
