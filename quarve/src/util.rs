#[cfg(debug_assertions)]
pub(crate) mod test_util {
    use std::marker::PhantomData;
    use std::sync::Mutex;

    #[cfg(debug_assertions)]
    static UNBALANCED_ALLOCS: Mutex<usize> = Mutex::new(0);

    pub struct QuarveAllocTag {
        private: PhantomData<i32>
    }

    impl QuarveAllocTag {
        pub fn new() -> QuarveAllocTag {
            #[cfg(debug_assertions)]
            {
                *UNBALANCED_ALLOCS.lock().unwrap() += 1;
            }

            QuarveAllocTag {
                private: PhantomData
            }
        }
    }

    #[cfg(debug_assertions)]
    impl Drop for QuarveAllocTag {
        fn drop(&mut self) {
            *UNBALANCED_ALLOCS.lock().unwrap() -= 1;
        }
    }

    #[cfg(debug_assertions)]
    pub struct HeapChecker {
        org_diff: usize
    }

    #[cfg(debug_assertions)]
    impl HeapChecker {
        #[allow(unused)]
        pub fn new() -> Self {
            HeapChecker {
                org_diff: *UNBALANCED_ALLOCS.lock().unwrap()
            }
        }

        #[allow(unused)]
        pub fn assert_diff(&self, diff: usize) {
            let curr = *UNBALANCED_ALLOCS.lock().unwrap();
            assert_eq!(curr - self.org_diff, diff);
        }
    }

    #[cfg(debug_assertions)]
    impl Drop for HeapChecker {
        fn drop(&mut self) {
            let curr = *UNBALANCED_ALLOCS.lock().unwrap();
            debug_assert_eq!(curr, self.org_diff, "Introduced Memory Leak")
        }
    }
}


pub(crate) struct UnsafeForceSend<T>(pub T);
unsafe impl<T> Send for UnsafeForceSend<T> {}

trait Attribute {
}

/*
struct AttributedString<T: Attribute> {

}
 */