fn main() {
    //                         (width considering line number) 80 cols -> |
    println!("*match to this line* {}", &["*match to fooooooooooo line*", "*match to this line*", "*match to this line*"]);
    println!("*match to this line* {}", &["*match to fooooooooooooooo line*", "*match to this line*", "*match to this line*"]);
}
