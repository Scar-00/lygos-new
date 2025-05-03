/// # Safety
/// this function is __REALLY__ unsafe and should be removed in the future
#[allow(clippy::mut_from_ref)]
pub unsafe fn to_mut<T>(r: &T) -> &mut T {
    unsafe{(r as *const T as *mut T).as_mut().unwrap()}
}

#[derive(Debug)]
pub struct Pointer<T: ?Sized> {
    inner: *const T,
}

impl<T> Default for Pointer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Pointer<T> {
    pub fn new() -> Self {
        Self{ inner: std::ptr::null() }
    }

    pub fn set_value(&mut self, value: &T) {
        self.inner = value as *const T;
    }

    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }
}

impl<T> From<&T> for Pointer<T> {
    fn from(value: &T) -> Self {
        Self{ inner: value as *const T }
    }
}

impl<T> From<&mut T> for Pointer<T> {
    fn from(value: &mut T) -> Self {
        Self{ inner: value as *const T }
    }
}

impl<T> From<&mut &mut T> for Pointer<T> {
    fn from(value: &mut &mut T) -> Self {
        Self{ inner: *value as *const T }
    }
}

impl<T> std::ops::Deref for Pointer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe{self.inner.as_ref()}.unwrap()
    }
}

impl<T> std::ops::DerefMut for Pointer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T> std::convert::AsRef<T> for Pointer<T> {
    fn as_ref(&self) -> &T {
        unsafe{ self.inner.as_ref().unwrap() }
    }
}

impl<T> std::convert::AsMut<T> for Pointer<T> {
    fn as_mut(&mut self) -> &mut T {
        unsafe{ self.inner.cast_mut().as_mut().unwrap() }
    }
}

impl<T> Clone for Pointer<T> {
    fn clone(&self) -> Self {
        Self{ inner: self.inner }
    }
}
