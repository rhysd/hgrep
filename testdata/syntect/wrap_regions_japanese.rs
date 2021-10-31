fn main() {
    //                         (width considering line number) 80 cols -> |
    println!("*match to あ line* {}", &["*match to foo line*", "*match to い line*", "*match to う line*"]);
    println!("*match to あ line* {}", &["*match to fo line*", "*match to い line*", "*match to う line*"]);
}
