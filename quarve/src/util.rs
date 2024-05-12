
pub(crate) struct UnsafeForceSend<T>(pub T);
unsafe impl<T> Send for UnsafeForceSend<T> {}

trait Attribute {
}

/*
struct AttributedString<T: Attribute> {

}
 */