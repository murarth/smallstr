use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    iter::FromIterator,
    ops, ptr, slice,
    str::{self, Chars, Utf8Error},
};

use alloc::{borrow::Cow, boxed::Box, string::String};

#[cfg(feature = "ffi")]
use std::ffi::{OsStr, OsString};

#[cfg(feature = "serde")]
use core::marker::PhantomData;
#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer, Error, Visitor},
    ser::{Serialize, Serializer},
};

use smallvec::{Array, SmallVec};

/// A `String`-like container that can store a small number of bytes inline.
///
/// `SmallString` uses a `SmallVec<[u8; N]>` as its internal storage.
#[derive(Clone, Default)]
pub struct SmallString<A: Array<Item = u8>> {
    data: SmallVec<A>,
}

impl<A: Array<Item = u8>> SmallString<A> {
    /// Construct an empty string.
    #[inline]
    pub fn new() -> SmallString<A> {
        SmallString {
            data: SmallVec::new(),
        }
    }

    /// Construct an empty string with enough capacity pre-allocated to store
    /// at least `n` bytes.
    ///
    /// Will create a heap allocation only if `n` is larger than the inline capacity.
    #[inline]
    pub fn with_capacity(n: usize) -> SmallString<A> {
        SmallString {
            data: SmallVec::with_capacity(n),
        }
    }

    /// Construct a `SmallString` by copying data from a `&str`.
    #[inline]
    pub fn from_str(s: &str) -> SmallString<A> {
        SmallString {
            data: SmallVec::from_slice(s.as_bytes()),
        }
    }

    /// Construct a `SmallString` by using an existing allocation.
    #[inline]
    pub fn from_string(s: String) -> SmallString<A> {
        SmallString {
            data: SmallVec::from_vec(s.into_bytes()),
        }
    }

    /// Constructs a new `SmallString` on the stack using UTF-8 bytes.
    ///
    /// If the provided byte array is not valid UTF-8, an error is returned.
    #[inline]
    pub fn from_buf(buf: A) -> Result<SmallString<A>, FromUtf8Error<A>> {
        let data = SmallVec::from_buf(buf);

        match str::from_utf8(&data) {
            Ok(_) => Ok(SmallString { data }),
            Err(error) => {
                let buf = data.into_inner().ok().unwrap();

                Err(FromUtf8Error { buf, error })
            }
        }
    }

    /// Constructs a new `SmallString` on the stack using the provided byte array
    /// without checking that the array contains valid UTF-8.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not check that the bytes passed
    /// to it are valid UTF-8. If this constraint is violated, it may cause
    /// memory unsafety issues, as the Rust standard library functions assume
    /// that `&str`s are valid UTF-8.
    #[inline]
    pub unsafe fn from_buf_unchecked(buf: A) -> SmallString<A> {
        SmallString {
            data: SmallVec::from_buf(buf),
        }
    }

    /// The maximum number of bytes this string can hold inline.
    #[inline]
    pub fn inline_size(&self) -> usize {
        A::size()
    }

    /// Returns the length of this string, in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if this string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the number of bytes this string can hold without reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns `true` if the data has spilled into a separate heap-allocated buffer.
    #[inline]
    pub fn spilled(&self) -> bool {
        self.data.spilled()
    }

    /// Empties the string and returns an iterator over its former contents.
    pub fn drain(&mut self) -> Drain {
        unsafe {
            let len = self.len();

            self.data.set_len(0);

            let ptr = self.as_ptr();

            let slice = slice::from_raw_parts(ptr, len);
            let s = str::from_utf8_unchecked(slice);

            Drain { iter: s.chars() }
        }
    }

    /// Appends the given `char` to the end of this string.
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<[u8; 8]> = SmallString::from("foo");
    ///
    /// s.push('x');
    ///
    /// assert_eq!(s, "foox");
    /// ```
    #[inline]
    pub fn push(&mut self, ch: char) {
        match ch.len_utf8() {
            1 => self.data.push(ch as u8),
            _ => self.push_str(ch.encode_utf8(&mut [0; 4])),
        }
    }

    /// Appends the given string slice to the end of this string.
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<[u8; 8]> = SmallString::from("foo");
    ///
    /// s.push_str("bar");
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.data.extend_from_slice(s.as_bytes());
    }

    /// Removes the last character from this string and returns it.
    ///
    /// Returns `None` if the string is empty.
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        match self.chars().next_back() {
            Some(ch) => unsafe {
                let new_len = self.len() - ch.len_utf8();
                self.data.set_len(new_len);
                Some(ch)
            },
            None => None,
        }
    }

    /// Reallocates to set the new capacity to `new_cap`.
    ///
    /// # Panics
    ///
    /// If `new_cap` is less than the current length.
    #[inline]
    pub fn grow(&mut self, new_cap: usize) {
        self.data.grow(new_cap);
    }

    /// Ensures that this string's capacity is at least `additional` bytes larger
    /// than its length.
    ///
    /// The capacity may be increased by more than `additional` bytes in order to
    /// prevent frequent reallocations.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Ensures that this string's capacity is `additional` bytes larger than
    /// its length.
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Shrink the capacity of the string as much as possible.
    ///
    /// When possible, this will move the data from an external heap buffer
    /// to the string's inline storage.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// Shorten the string, keeping the first `len` bytes.
    ///
    /// This does not reallocate. If you want to shrink the string's capacity,
    /// use `shrink_to_fit` after truncating.
    ///
    /// # Panics
    ///
    /// If `len` does not lie on a `char` boundary.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        assert!(self.is_char_boundary(len));
        self.data.truncate(len);
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self
    }

    /// Extracts a string slice containing the entire string.
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self
    }

    /// Removes all contents of the string.
    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Removes a `char` from this string at a byte position and returns it.
    ///
    /// # Panics
    ///
    /// If `idx` does not lie on a `char` boundary.
    #[inline]
    pub fn remove(&mut self, idx: usize) -> char {
        let ch = match self[idx..].chars().next() {
            Some(ch) => ch,
            None => panic!("cannot remove a char from the end of a string"),
        };

        let ch_len = ch.len_utf8();
        let next = idx + ch_len;
        let len = self.len();

        unsafe {
            ptr::copy(
                self.as_ptr().add(next),
                self.as_mut_ptr().add(idx),
                len - next,
            );
            self.data.set_len(len - ch_len);
        }

        ch
    }

    /// Inserts a `char` into this string at the given byte position.
    ///
    /// # Panics
    ///
    /// If `idx` does not lie on `char` boundaries.
    #[inline]
    pub fn insert(&mut self, idx: usize, ch: char) {
        assert!(self.is_char_boundary(idx));

        match ch.len_utf8() {
            1 => self.data.insert(idx, ch as u8),
            _ => self.insert_str(idx, ch.encode_utf8(&mut [0; 4])),
        }
    }

    /// Inserts a `&str` into this string at the given byte position.
    ///
    /// # Panics
    ///
    /// If `idx` does not lie on `char` boundaries.
    #[inline]
    pub fn insert_str(&mut self, idx: usize, s: &str) {
        assert!(self.is_char_boundary(idx));

        let len = self.len();
        let amt = s.len();

        self.data.reserve(amt);

        unsafe {
            ptr::copy(
                self.as_ptr().add(idx),
                self.as_mut_ptr().add(idx + amt),
                len - idx,
            );
            ptr::copy_nonoverlapping(s.as_ptr(), self.as_mut_ptr().add(idx), amt);
            self.data.set_len(len + amt);
        }
    }

    /// Returns a mutable reference to the contents of the `SmallString`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not check that the bytes passed
    /// to it are valid UTF-8. If this constraint is violated, it may cause
    /// memory unsafety issues, as the Rust standard library functions assume
    /// that `&str`s are valid UTF-8.
    #[inline]
    pub unsafe fn as_mut_vec(&mut self) -> &mut SmallVec<A> {
        &mut self.data
    }

    /// Converts the `SmallString` into a `String`, without reallocating if the
    /// `SmallString` has already spilled onto the heap.
    #[inline]
    pub fn into_string(self) -> String {
        unsafe { String::from_utf8_unchecked(self.data.into_vec()) }
    }

    /// Converts the `SmallString` into a `Box<str>`, without reallocating if the
    /// `SmallString` has already spilled onto the heap.
    ///
    /// Note that this will drop excess capacity.
    #[inline]
    pub fn into_boxed_str(self) -> Box<str> {
        self.into_string().into_boxed_str()
    }

    /// Convert the `SmallString` into `A`, if possible. Otherwise, return `Err(self)`.
    ///
    /// This method returns `Err(self)` if the `SmallString` is too short
    /// (and the `A` contains uninitialized elements) or if the `SmallString` is too long
    /// (and the elements have been spilled to the heap).
    #[inline]
    pub fn into_inner(self) -> Result<A, Self> {
        self.data.into_inner().map_err(|data| SmallString { data })
    }

    /// Retains only the characters specified by the predicate.
    ///
    /// In other words, removes all characters `c` such that `f(c)` returns `false`.
    /// This method operates in place and preserves the order of retained
    /// characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<[u8; 16]> = SmallString::from("f_o_ob_ar");
    ///
    /// s.retain(|c| c != '_');
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    #[inline]
    pub fn retain<F: FnMut(char) -> bool>(&mut self, mut f: F) {
        struct SetLenOnDrop<'a, A: Array<Item = u8>> {
            s: &'a mut SmallString<A>,
            idx: usize,
            del_bytes: usize,
        }

        impl<'a, A: Array<Item = u8>> Drop for SetLenOnDrop<'a, A> {
            fn drop(&mut self) {
                let new_len = self.idx - self.del_bytes;
                debug_assert!(new_len <= self.s.len());
                unsafe { self.s.data.set_len(new_len) };
            }
        }

        let len = self.len();
        let mut guard = SetLenOnDrop {
            s: self,
            idx: 0,
            del_bytes: 0,
        };

        while guard.idx < len {
            let ch = unsafe {
                guard
                    .s
                    .get_unchecked(guard.idx..len)
                    .chars()
                    .next()
                    .unwrap()
            };
            let ch_len = ch.len_utf8();

            if !f(ch) {
                guard.del_bytes += ch_len;
            } else if guard.del_bytes > 0 {
                unsafe {
                    ptr::copy(
                        guard.s.data.as_ptr().add(guard.idx),
                        guard.s.data.as_mut_ptr().add(guard.idx - guard.del_bytes),
                        ch_len,
                    );
                }
            }

            // Point idx to the next char
            guard.idx += ch_len;
        }

        drop(guard);
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.as_ptr() as *mut u8
    }
}

impl<A: Array<Item = u8>> ops::Deref for SmallString<A> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        let bytes: &[u8] = &self.data;
        unsafe { str::from_utf8_unchecked(bytes) }
    }
}

impl<A: Array<Item = u8>> ops::DerefMut for SmallString<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        let bytes: &mut [u8] = &mut self.data;
        unsafe { str::from_utf8_unchecked_mut(bytes) }
    }
}

impl<A: Array<Item = u8>> AsRef<str> for SmallString<A> {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}

impl<A: Array<Item = u8>> AsMut<str> for SmallString<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut str {
        self
    }
}

impl<A: Array<Item = u8>> Borrow<str> for SmallString<A> {
    #[inline]
    fn borrow(&self) -> &str {
        self
    }
}

impl<A: Array<Item = u8>> BorrowMut<str> for SmallString<A> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut str {
        self
    }
}

impl<A: Array<Item = u8>> AsRef<[u8]> for SmallString<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.data.as_ref()
    }
}

impl<A: Array<Item = u8>> Borrow<[u8]> for SmallString<A> {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.data.borrow()
    }
}

impl<A: Array<Item = u8>> fmt::Write for SmallString<A> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, ch: char) -> fmt::Result {
        self.push(ch);
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl<A: Array<Item = u8>> Serialize for SmallString<A> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de, A: Array<Item = u8>> Deserialize<'de> for SmallString<A> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(SmallStringVisitor {
            phantom: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
struct SmallStringVisitor<A> {
    phantom: PhantomData<A>,
}

#[cfg(feature = "serde")]
impl<'de, A: Array<Item = u8>> Visitor<'de> for SmallStringVisitor<A> {
    type Value = SmallString<A>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.into())
    }

    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(v.into())
    }
}

impl<A: Array<Item = u8>> From<char> for SmallString<A> {
    #[inline]
    fn from(ch: char) -> SmallString<A> {
        SmallString::from_str(ch.encode_utf8(&mut [0; 4]))
    }
}

impl<'a, A: Array<Item = u8>> From<&'a str> for SmallString<A> {
    #[inline]
    fn from(s: &str) -> SmallString<A> {
        SmallString::from_str(s)
    }
}

impl<A: Array<Item = u8>> From<Box<str>> for SmallString<A> {
    #[inline]
    fn from(s: Box<str>) -> SmallString<A> {
        SmallString::from_string(s.into())
    }
}

impl<A: Array<Item = u8>> From<String> for SmallString<A> {
    #[inline]
    fn from(s: String) -> SmallString<A> {
        SmallString::from_string(s)
    }
}

macro_rules! impl_index_str {
    ($index_type: ty) => {
        impl<A: Array<Item = u8>> ops::Index<$index_type> for SmallString<A> {
            type Output = str;

            #[inline]
            fn index(&self, index: $index_type) -> &str {
                &self.as_str()[index]
            }
        }

        impl<A: Array<Item = u8>> ops::IndexMut<$index_type> for SmallString<A> {
            #[inline]
            fn index_mut(&mut self, index: $index_type) -> &mut str {
                &mut self.as_mut_str()[index]
            }
        }
    };
}

impl_index_str!(ops::Range<usize>);
impl_index_str!(ops::RangeFrom<usize>);
impl_index_str!(ops::RangeTo<usize>);
impl_index_str!(ops::RangeFull);

impl<A: Array<Item = u8>> FromIterator<char> for SmallString<A> {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> SmallString<A> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<'a, A: Array<Item = u8>> FromIterator<&'a char> for SmallString<A> {
    fn from_iter<I: IntoIterator<Item = &'a char>>(iter: I) -> SmallString<A> {
        let mut s = SmallString::new();
        s.extend(iter.into_iter().cloned());
        s
    }
}

impl<'a, A: Array<Item = u8>> FromIterator<Cow<'a, str>> for SmallString<A> {
    fn from_iter<I: IntoIterator<Item = Cow<'a, str>>>(iter: I) -> SmallString<A> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<'a, A: Array<Item = u8>> FromIterator<&'a str> for SmallString<A> {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> SmallString<A> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<A: Array<Item = u8>> FromIterator<String> for SmallString<A> {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> SmallString<A> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<A: Array<Item = u8>> Extend<char> for SmallString<A> {
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (lo, _) = iter.size_hint();

        self.reserve(lo);

        for ch in iter {
            self.push(ch);
        }
    }
}

impl<'a, A: Array<Item = u8>> Extend<&'a char> for SmallString<A> {
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }
}

impl<'a, A: Array<Item = u8>> Extend<Cow<'a, str>> for SmallString<A> {
    fn extend<I: IntoIterator<Item = Cow<'a, str>>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl<'a, A: Array<Item = u8>> Extend<&'a str> for SmallString<A> {
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(s);
        }
    }
}

impl<A: Array<Item = u8>> Extend<String> for SmallString<A> {
    fn extend<I: IntoIterator<Item = String>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl<A: Array<Item = u8>> fmt::Debug for SmallString<A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<A: Array<Item = u8>> fmt::Display for SmallString<A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

macro_rules! eq_str {
    ( $rhs:ty ) => {
        impl<'a, A: Array<Item = u8>> PartialEq<$rhs> for SmallString<A> {
            #[inline]
            fn eq(&self, rhs: &$rhs) -> bool {
                &self[..] == &rhs[..]
            }

            #[inline]
            fn ne(&self, rhs: &$rhs) -> bool {
                &self[..] != &rhs[..]
            }
        }
    };
}

eq_str!(str);
eq_str!(&'a str);
eq_str!(String);
eq_str!(Cow<'a, str>);

#[cfg(feature = "ffi")]
impl<A: Array<Item = u8>> PartialEq<OsStr> for SmallString<A> {
    #[inline]
    fn eq(&self, rhs: &OsStr) -> bool {
        &self[..] == rhs
    }

    #[inline]
    fn ne(&self, rhs: &OsStr) -> bool {
        &self[..] != rhs
    }
}

#[cfg(feature = "ffi")]
impl<'a, A: Array<Item = u8>> PartialEq<&'a OsStr> for SmallString<A> {
    #[inline]
    fn eq(&self, rhs: &&OsStr) -> bool {
        &self[..] == *rhs
    }

    #[inline]
    fn ne(&self, rhs: &&OsStr) -> bool {
        &self[..] != *rhs
    }
}

#[cfg(feature = "ffi")]
impl<A: Array<Item = u8>> PartialEq<OsString> for SmallString<A> {
    #[inline]
    fn eq(&self, rhs: &OsString) -> bool {
        &self[..] == rhs
    }

    #[inline]
    fn ne(&self, rhs: &OsString) -> bool {
        &self[..] != rhs
    }
}

#[cfg(feature = "ffi")]
impl<'a, A: Array<Item = u8>> PartialEq<Cow<'a, OsStr>> for SmallString<A> {
    #[inline]
    fn eq(&self, rhs: &Cow<OsStr>) -> bool {
        self[..] == **rhs
    }

    #[inline]
    fn ne(&self, rhs: &Cow<OsStr>) -> bool {
        self[..] != **rhs
    }
}

impl<A, B> PartialEq<SmallString<B>> for SmallString<A>
where
    A: Array<Item = u8>,
    B: Array<Item = u8>,
{
    #[inline]
    fn eq(&self, rhs: &SmallString<B>) -> bool {
        &self[..] == &rhs[..]
    }

    #[inline]
    fn ne(&self, rhs: &SmallString<B>) -> bool {
        &self[..] != &rhs[..]
    }
}

impl<A: Array<Item = u8>> Eq for SmallString<A> {}

impl<A: Array<Item = u8>> PartialOrd for SmallString<A> {
    #[inline]
    fn partial_cmp(&self, rhs: &SmallString<A>) -> Option<Ordering> {
        self[..].partial_cmp(&rhs[..])
    }
}

impl<A: Array<Item = u8>> Ord for SmallString<A> {
    #[inline]
    fn cmp(&self, rhs: &SmallString<A>) -> Ordering {
        self[..].cmp(&rhs[..])
    }
}

impl<A: Array<Item = u8>> Hash for SmallString<A> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self[..].hash(state)
    }
}

/// A draining iterator for `SmallString`.
///
/// This struct is created by the [`drain`] method on [`SmallString`].
///
/// [`drain`]: struct.SmallString.html#method.drain
/// [`SmallString`]: struct.SmallString.html
pub struct Drain<'a> {
    iter: Chars<'a>,
}

impl<'a> Iterator for Drain<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for Drain<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        self.iter.next_back()
    }
}

/// A possible error value when creating a `SmallString` from a byte array.
///
/// This type is the error type for the [`from_buf`] method on [`SmallString`].
///
/// [`from_buf`]: struct.SmallString.html#method.from_buf
/// [`SmallString`]: struct.SmallString.html
#[derive(Debug)]
pub struct FromUtf8Error<A: Array<Item = u8>> {
    buf: A,
    error: Utf8Error,
}

impl<A: Array<Item = u8>> FromUtf8Error<A> {
    /// Returns the slice of `[u8]` bytes that were attempted to convert to a `SmallString`.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        let ptr = &self.buf as *const _ as *const u8;
        unsafe { slice::from_raw_parts(ptr, A::size()) }
    }

    /// Returns the byte array that was attempted to convert into a `SmallString`.
    #[inline]
    pub fn into_buf(self) -> A {
        self.buf
    }

    /// Returns the `Utf8Error` to get more details about the conversion failure.
    #[inline]
    pub fn utf8_error(&self) -> Utf8Error {
        self.error
    }
}

impl<A: Array<Item = u8>> fmt::Display for FromUtf8Error<A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

#[cfg(test)]
mod test {
    use alloc::{
        borrow::{Cow, ToOwned},
        string::{String, ToString},
    };

    use super::SmallString;

    #[test]
    fn test_drain() {
        let mut s: SmallString<[u8; 2]> = SmallString::new();

        s.push('a');
        assert_eq!(s.drain().collect::<String>(), "a");
        assert!(s.is_empty());

        // spilling the vec
        s.push('x');
        s.push('y');
        s.push('z');

        assert_eq!(s.drain().collect::<String>(), "xyz");
        assert!(s.is_empty());
    }

    #[test]
    fn test_drain_rev() {
        let mut s: SmallString<[u8; 2]> = SmallString::new();

        s.push('a');
        assert_eq!(s.drain().rev().collect::<String>(), "a");
        assert!(s.is_empty());

        // spilling the vec
        s.push('x');
        s.push('y');
        s.push('z');

        assert_eq!(s.drain().rev().collect::<String>(), "zyx");
        assert!(s.is_empty());
    }

    #[test]
    fn test_eq() {
        let s: SmallString<[u8; 4]> = SmallString::from("foo");

        assert_eq!(s, *"foo");
        assert_eq!(s, "foo");
        assert_eq!(s, "foo".to_owned());
        assert_eq!(s, Cow::Borrowed("foo"));
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn test_eq_os_str() {
        use std::ffi::OsStr;

        let s: SmallString<[u8; 4]> = SmallString::from("foo");
        let os_s: &OsStr = "foo".as_ref();

        assert_eq!(s, os_s);
        assert_eq!(s, *os_s);
        assert_eq!(s, os_s.to_owned());
        assert_eq!(s, Cow::Borrowed(os_s));
    }

    #[test]
    fn test_from_buf() {
        let s: SmallString<[u8; 2]> = SmallString::from_buf([206, 177]).unwrap();
        assert_eq!(s, "α");

        assert!(SmallString::<[u8; 2]>::from_buf([206, 0]).is_err());
    }

    #[test]
    fn test_insert() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("abc");

        s.insert(1, 'x');
        assert_eq!(s, "axbc");

        s.insert(3, 'α');
        assert_eq!(s, "axbαc");

        s.insert_str(0, "foo");
        assert_eq!(s, "fooaxbαc");
    }

    #[test]
    #[should_panic]
    fn test_insert_panic() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("αβγ");

        s.insert(1, 'x');
    }

    #[test]
    fn test_into_string() {
        let s: SmallString<[u8; 2]> = SmallString::from("foo");
        assert_eq!(s.into_string(), "foo");

        let s: SmallString<[u8; 8]> = SmallString::from("foo");
        assert_eq!(s.into_string(), "foo");
    }

    #[test]
    fn test_to_string() {
        let s: SmallString<[u8; 2]> = SmallString::from("foo");
        assert_eq!(s.to_string(), "foo");

        let s: SmallString<[u8; 8]> = SmallString::from("foo");
        assert_eq!(s.to_string(), "foo");
    }

    #[test]
    fn test_pop() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("αβγ");

        assert_eq!(s.pop(), Some('γ'));
        assert_eq!(s.pop(), Some('β'));
        assert_eq!(s.pop(), Some('α'));
        assert_eq!(s.pop(), None);
    }

    #[test]
    fn test_remove() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("αβγ");

        assert_eq!(s.remove(2), 'β');
        assert_eq!(s, "αγ");

        assert_eq!(s.remove(0), 'α');
        assert_eq!(s, "γ");

        assert_eq!(s.remove(0), 'γ');
        assert_eq!(s, "");
    }

    #[test]
    #[should_panic]
    fn test_remove_panic_0() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("foo");

        // Attempt to remove at the end
        s.remove(3);
    }

    #[test]
    #[should_panic]
    fn test_remove_panic_1() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("αβγ");

        // Attempt to remove mid-character
        s.remove(1);
    }

    #[test]
    fn test_retain() {
        let mut s: SmallString<[u8; 8]> = SmallString::from("α_β_γ");

        s.retain(|_| true);
        assert_eq!(s, "α_β_γ");

        s.retain(|c| c != '_');
        assert_eq!(s, "αβγ");

        s.retain(|c| c != 'β');
        assert_eq!(s, "αγ");

        s.retain(|c| c == 'α');
        assert_eq!(s, "α");

        s.retain(|_| false);
        assert_eq!(s, "");
    }

    #[test]
    fn test_truncate() {
        let mut s: SmallString<[u8; 2]> = SmallString::from("foobar");

        s.truncate(6);
        assert_eq!(s, "foobar");

        s.truncate(3);
        assert_eq!(s, "foo");
    }

    #[test]
    #[should_panic]
    fn test_truncate_panic() {
        let mut s: SmallString<[u8; 2]> = SmallString::from("α");

        s.truncate(1);
    }

    #[test]
    fn test_write() {
        use core::fmt::Write;

        let mut s: SmallString<[u8; 8]> = SmallString::from("foo");

        write!(s, "bar").unwrap();

        assert_eq!(s, "foobar");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        use bincode::{deserialize, serialize};

        let mut small_str: SmallString<[u8; 4]> = SmallString::from("foo");

        let encoded = serialize(&small_str).unwrap();
        let decoded: SmallString<[u8; 4]> = deserialize(&encoded).unwrap();

        assert_eq!(small_str, decoded);

        // Spill the vec
        small_str.push_str("bar");

        // Check again after spilling.
        let encoded = serialize(&small_str).unwrap();
        let decoded: SmallString<[u8; 4]> = deserialize(&encoded).unwrap();

        assert_eq!(small_str, decoded);
    }
}
