use is_terminal::IsTerminal;

use crate::cli::GlobalOpts;

pub fn is_color_enabled(global: &GlobalOpts) -> bool {
    if global.format.is_json() || global.no_color {
        return false;
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    std::io::stdout().is_terminal()
}
