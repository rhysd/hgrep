use flate2::write::ZlibEncoder;
use path_slash::PathBufExt;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use syntect::highlighting::ThemeSet;

const THEME_PATHS: &[&str] = &[
    "../assets/ansi.tmTheme",
    "../assets/submodules/1337-Scheme/1337.tmTheme",
    "../assets/submodules/Nord-plist/Nord.tmTheme",
    "../assets/submodules/Solarized/Solarized (dark).tmTheme",
    "../assets/submodules/Solarized/Solarized (light).tmTheme",
    "../assets/submodules/TwoDark/TwoDark.tmTheme",
    "../assets/submodules/ayu/ayu-light.tmTheme",
    "../assets/submodules/ayu/ayu-mirage.tmTheme",
    "../assets/submodules/ayu/ayu-dark.tmTheme",
    "../assets/submodules/coldark-bat/Coldark-Dark.tmTheme",
    "../assets/submodules/cyanide-theme/Cyanide.tmTheme",
    "../assets/submodules/github-sublime-theme/GitHub.tmTheme",
    "../assets/submodules/gruvbox-tmTheme/gruvbox-dark.tmTheme",
    "../assets/submodules/gruvbox-tmTheme/gruvbox-light.tmTheme",
    "../assets/submodules/onehalf/sublimetext/OneHalfDark.tmTheme",
    "../assets/submodules/onehalf/sublimetext/OneHalfLight.tmTheme",
    "../assets/submodules/predawn/predawn.tmTheme",
    "../assets/submodules/sublime-monokai-extended/Monokai Extended Bright.tmTheme",
    "../assets/submodules/sublime-monokai-extended/Monokai Extended Light.tmTheme",
    "../assets/submodules/sublime-monokai-extended/Monokai Extended.tmTheme",
    "../assets/submodules/sublime-snazzy/Sublime Snazzy.tmTheme",
    "../assets/submodules/sublime/Dracula.tmTheme",
    "../assets/submodules/visual-studio-dark-plus/Visual Studio Dark+.tmTheme",
    "../assets/submodules/zenburn/zenburn.tmTheme",
];

const THEME_BIN_PATH: &str = "../assets/themes.bin";

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
