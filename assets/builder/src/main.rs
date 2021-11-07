use flate2::write::ZlibEncoder;
use path_slash::PathBufExt;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use syntect::highlighting::ThemeSet;

const THEME_PATHS: &[&str] = &[
    "../ansi.tmTheme",
    "../submodules/1337-Scheme/1337.tmTheme",
    "../submodules/Nord-plist/Nord.tmTheme",
    "../submodules/Solarized/Solarized (dark).tmTheme",
    "../submodules/Solarized/Solarized (light).tmTheme",
    "../submodules/TwoDark/TwoDark.tmTheme",
    "../submodules/ayu/ayu-light.tmTheme",
    "../submodules/ayu/ayu-mirage.tmTheme",
    "../submodules/ayu/ayu-dark.tmTheme",
    "../submodules/coldark-bat/Coldark-Dark.tmTheme",
    "../submodules/cyanide-theme/Cyanide.tmTheme",
    "../submodules/github-sublime-theme/GitHub.tmTheme",
    "../submodules/gruvbox-tmTheme/gruvbox-dark.tmTheme",
    "../submodules/gruvbox-tmTheme/gruvbox-light.tmTheme",
    "../submodules/onehalf/sublimetext/OneHalfDark.tmTheme",
    "../submodules/onehalf/sublimetext/OneHalfLight.tmTheme",
    "../submodules/predawn/predawn.tmTheme",
    "../submodules/sublime-monokai-extended/Monokai Extended Bright.tmTheme",
    "../submodules/sublime-monokai-extended/Monokai Extended Light.tmTheme",
    "../submodules/sublime-monokai-extended/Monokai Extended.tmTheme",
    "../submodules/sublime-snazzy/Sublime Snazzy.tmTheme",
    "../submodules/sublime/Dracula.tmTheme",
    "../submodules/visual-studio-dark-plus/Visual Studio Dark+.tmTheme",
    "../submodules/zenburn/zenburn.tmTheme",
    "../submodules/material-theme/schemes/Material-Theme.tmTheme",
];

const THEME_BIN_PATH: &str = "../themes.bin";

fn main() {
    println!("Building theme set for syntect-printer: {}", THEME_BIN_PATH);

    let mut set = ThemeSet::new();

    for path in THEME_PATHS {
        let path = PathBuf::from_slash(path);
        println!("Loading theme from {:?}", path);

        let name = path.file_stem().and_then(OsStr::to_str).expect("File stem was not found in .tmTheme file. Did you specify incorrect file in THEME_PATHS?");
        let theme = ThemeSet::get_theme(&path).expect("Theme file was not found. Did you forget fetching submodules in ./submodules directory?");
        set.themes.insert(name.to_string(), theme);

        println!("Loaded theme from {:?}", path);
    }

    println!("Compressing theme set");
    let mut buf = vec![];
    bincode::serialize_into(
        ZlibEncoder::new(&mut buf, flate2::Compression::best()),
        &set,
    )
    .expect("Theme set could not be compressed with bincode and flate2");

    println!(
        "Writing compressed theme set to {} ({} bytes)",
        THEME_BIN_PATH,
        buf.len()
    );
    fs::write(PathBuf::from_slash(THEME_BIN_PATH), &buf)
        .expect("Could not write compressed theme set");

    println!("Built successfully: {}", THEME_BIN_PATH);
}
