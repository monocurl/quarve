// TODO

mod attribute {
    mod character {
        use crate::view::util::Color;

        pub struct Bold;

        pub struct Italic;

        pub struct Underline;

        pub struct Strikethrough;

        pub struct BackColor(pub Color);

        pub struct ForeColor(pub Color);

        pub struct Font;
    }

    mod run {
        pub struct Justification;
        pub struct Indentation {

        }
    }

    mod document {

    }
}

struct Text {

}

struct TextField {

}