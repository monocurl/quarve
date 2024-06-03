pub(crate) mod rust_util {
    use std::marker::PhantomData;

    #[allow(unused)]
    pub trait EnsureSend: Send {

    }

    #[allow(unused)]
    pub trait EnsureSync: Sync {

    }

    pub type PhantomUnsendUnsync = PhantomData<*const ()>;
}


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

mod vector {
    use std::fmt::Debug;
    use std::ops::{Add, Mul, Sub};
    use crate::state::Stateful;
    use crate::util::numeric::{Lerp, Norm};

    // G^N
    #[derive(Copy, Clone, Debug)]
    pub struct Vector<T, const N: usize>(pub [T; N]) where T: Stateful;

    macro_rules! vector_get_set {
        ($I:expr, $get:ident, $set:ident, $N:expr) => {
            impl<T> Vector<T, $N> where T: Stateful {
                pub fn $get(&self) -> &T {
                    &self.0[$I]
                }

                pub fn $set(&mut self, val: T) {
                    self.0[$I] = val;
                }
            }
        };

        ($i:expr, $get:ident, $set:ident, $N:expr, $($N1:expr), +) => {
            vector_get_set!($i, $get, $set, $N);
            vector_get_set!($i, $get, $set, $($N1),+);
        };
    }

    vector_get_set!(0, x, set_x, 1, 2, 3, 4);
    vector_get_set!(1, y, set_y, 2, 3, 4);
    vector_get_set!(2, z, set_z, 3, 4);
    vector_get_set!(3, w, set_w, 4);

    impl<T, const N: usize> Vector<T, N> where T: Stateful {
        pub fn from_array(arr: [T; N]) -> Self {
            Vector(arr)
        }
    }

    // scalar multiplication, addition, and multiplication are provided
    impl<T, const N: usize> Add for Vector<T, N>
        where T: Stateful,
              T: Add<Output=T> + Copy {
        type Output = Vector<T, N>;

        fn add(mut self, rhs: Vector<T, N>) -> Self::Output {
            let mut i = 0;
            self.0 = self.0.map(|x| {
                let ret = x + rhs.0[i];
                i += 1;
                ret
            });

            self
        }
    }

    impl<T, const N: usize> Sub for Vector<T, N>
        where T: Stateful,
              T: Sub<Output=T> + Copy {
        type Output = Vector<T, N>;

        fn sub(mut self, rhs: Vector<T, N>) -> Self::Output {
            let mut i = 0;
            self.0 = self.0.map(|x| {
                let ret = x - rhs.0[i];
                i += 1;
                ret
            });

            self
        }
    }

    // right scalar multiplication
    impl<T, const N: usize> Mul<T> for Vector<T, N>
        where T: Stateful,
              T: Mul<Output=T> + Copy {
        type Output = Vector<T, N>;

        fn mul(mut self, rhs: T) -> Self::Output {
            let mut i = 0;
            self.0 = self.0.map(|x| {
                let ret = x * rhs;
                i += 1;
                ret
            });

            self
        }
    }

    // left scalar multiplication
    macro_rules! impl_left_mult {
        ($($t:ty), *) => {
            $(
            impl<const N: usize> Mul<Vector<$t, N>> for $t {
                type Output = Vector<$t, N>;

                fn mul(self, mut rhs: Vector<$t, N>) -> Self::Output {
                    let mut i = 0;
                    rhs.0 = rhs.0.map(|x| {
                        let ret = self * x;
                        i += 1;
                        ret
                    });

                    rhs
                }
            }
            )*
        };
    }

    impl_left_mult!(
        u8, i8,
        u16, i16,
        u32, i32,
        u64, i64,
        usize, isize,
        f32, f64
    );

    impl<T, const N: usize> Lerp for Vector<T, N>
        where T: Stateful + Copy + Lerp + Debug
    {
        fn lerp(mut lhs: Self, factor: f64, rhs: Self) -> Self {
            for i in 0 .. N {
                lhs.0[i] = T::lerp(lhs.0[i], factor, rhs.0[i]);
            }

            lhs
        }
    }

    impl<T, const N: usize> Norm for Vector<T, N>
        where T: Stateful + Copy + Mul<Output=T> + Into<f64>
    {
        fn norm(&self) -> f64 {
            let mut sum = 0.0;
            for i in 0 .. N {
                sum += (self.0[i] * self.0[i]).into()
            }

            sum.sqrt()
        }
    }
}
pub use vector::*;


pub mod numeric {
    pub trait Lerp
    {
        fn lerp(lhs: Self, factor: f64, rhs: Self) -> Self;
    }

    macro_rules! impl_interpolatable {
        ($($t:ty), *) => {
            $(
                impl Lerp for $t {
                    fn lerp(lhs: Self, factor: f64, rhs: Self) -> Self {
                         ((lhs as f64) * (1.0 - factor) + (rhs as f64) * factor) as $t
                    }
                }
            )*
        };
    }

    impl_interpolatable!(
        i8, u8,
        i16, u16,
        i32, u32,
        i64, u64,
        isize, usize,
        f32, f64
    );

    pub trait Norm {
        fn norm(&self) -> f64;
    }

    macro_rules! impl_unsigned_norm {
        ($($t:ty), *) => {
            $(
                impl Norm for $t {
                    fn norm(&self) -> f64 {
                        *self as f64
                    }
                }
            )*
        };
    }

    macro_rules! impl_signed_norm {
        ($($t:ty), *) => {
            $(
                impl Norm for $t {
                    fn norm(&self) -> f64 {
                        self.abs() as f64
                    }
                }
            )*
        };
    }

    impl_unsigned_norm!(
        u8,
        u16,
        u32,
        u64,
        usize
    );

    impl_signed_norm!(
        i8,
        i16,
        i32,
        i64,
        isize,
        f32, f64
    );
}

pub mod markers {
    use crate::util::markers::sealed_base::BoolMarkerBase;

    mod sealed_base {
        pub trait BoolMarkerBase: Copy {

        }

        pub trait ThreadMarkerBase: Copy {

        }
    }

    pub trait BoolMarker: BoolMarkerBase { }
    #[derive(Copy, Clone)]
    pub struct TrueMarker { }
    #[derive(Copy, Clone)]
    pub struct FalseMarker { }

    impl BoolMarkerBase for TrueMarker { }
    impl BoolMarkerBase for FalseMarker { }
    impl BoolMarker for TrueMarker { }
    impl BoolMarker for FalseMarker { }

    pub trait ThreadMarker: sealed_base::ThreadMarkerBase {}
    #[derive(Copy, Clone)]
    pub struct AnyThreadMarker;
    #[derive(Copy, Clone)]
    pub struct MainThreadMarker;

    impl sealed_base::ThreadMarkerBase for AnyThreadMarker {}
    impl ThreadMarker for AnyThreadMarker {}
    impl sealed_base::ThreadMarkerBase for MainThreadMarker {}
    impl ThreadMarker for MainThreadMarker {}
}

pub mod geo {
    pub type ScreenUnit = f32;

    #[derive(Copy, Clone, Default)]
    pub struct Rect {
        pub x: ScreenUnit,
        pub y: ScreenUnit,
        pub w: ScreenUnit,
        pub h: ScreenUnit,
    }


    #[derive(Copy, Clone, Default)]
    pub struct Point {
        pub x: ScreenUnit,
        pub y: ScreenUnit,
    }

    #[derive(Copy, Clone, Default)]
    pub struct Size {
        pub w: ScreenUnit,
        pub h: ScreenUnit,
    }

    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum Alignment {
        TopLeading,
        Top,
        TopTrailing,
        Leading,
        Center,
        Trailing,
        BotLeading,
        Bot,
        BotTrailing,
    }

    impl Default for Alignment {
        fn default() -> Self {
            Self::Center
        }
    }

    #[derive(Copy, Clone, Default, PartialEq)]
    pub struct AlignedFrame {
        pub w: ScreenUnit,
        pub h: ScreenUnit,
        pub align: Alignment
    }

    impl AlignedFrame {
        pub fn full_rect(self) -> Rect {
            Rect {
                x: 0.0,
                y: 0.0,
                w: self.w,
                h: self.h,
            }
        }
    }
}
