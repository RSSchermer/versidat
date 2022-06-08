use std::cell::{Cell, UnsafeCell};
use std::ops::{Deref, DerefMut};
use std::{fmt, marker};

use crate::store::{ReadContext, UpdateContext};

// We basically reimplement RefCell, but as a type that is allowed to be Send and Sync. The store
// allows multiple "read" scopes to be alive across different threads, but only allows a single
// "update" scope. Updating a versioned scope requires an "update context" - scoped to that same
// update scope - be passed as proof. This constrains updates to a single thread and at the same
// time ensures there is no additional aliasing possible outside of this scope. Hence inside an
// update scope a versioned cell can expose sound interior mutability through the same
// borrow-tracking mechanisms used by RefCell. At the same time, in read scopes, it is guaranteed
// that no writes will occur, so the data inside the versioned cell can be dereferenced at will
// without runtime borrow tracking.

// Much of this is copied/modified from `core::cell`.

#[derive(Debug)]
pub struct BorrowError {}

impl fmt::Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt("already mutably borrowed", f)
    }
}

#[derive(Debug)]
pub struct BorrowMutError {}

impl fmt::Display for BorrowMutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt("already borrowed", f)
    }
}

/// Cell that changes it's version number whenever the data inside is mutably borrowed.
///
/// A [VersionedCell] can only be created during a [Store] "update scope" (see [Store::update]) and
/// can only outlive that scope by being stored as part of the store's data graph. Thus, a
/// [VersionedCell] is tied to its store for the entirety of its lifetime. It can only be accessed
/// through its store inside of a "read scope" (see [Store::with]) or inside of an "update scope"
/// (see [Store::update]).
///
/// Can be dereferenced during a read scope with [deref]; this requires passing the read scope's
/// [ReadContext] as proof. Dereferencing a [VersionedCell] like this incurs no additional overhead.
///
/// Inside of an update scope, a [VersionedCell] behaves like a [RefCell]. Its data can be borrowed
/// with [borrow] and mutably borrowed with [borrow_mut]. Both required the update scope's
/// [UpdateContext] as proof. Borrows are tracked:
///
/// - Borrowing a [VersionedCell] while a mutable borrow is life will result in a panic.
/// - Mutably borrowing a [VersionedCell] while any other borrow is life will result in a panic.
///
/// See [try_borrow] and [try_borrow_mut] respectively for non-panicking alternatives.
///
/// Mutably borrowing the [VersionedCell] through [borrow_mut] or [try_borrow_mut] will result in
/// it's version number changing. Note that version numbers on individual cells don't necessarily
/// increase monotonously. Instead, a "store global" version number is maintained across the store,
/// that is updated whenever any [VersionedCell] in the store is mutably borrowed, or when a new
/// [VersionedCell] is created. This ensures that when a [VersionedCell] at some location in the
/// store data-graph is replaced in its entirety by another [VersionedCell], this can be observed
/// later as a change in the version number of the [VersionedCell] at that location in the
/// data-graph (a new [VersionedCell] is guaranteed to never have the same version number as any
/// prior cell in the store at any point in time).
pub struct VersionedCell<'store, T: 'store + ?Sized> {
    // Note: don't need atomics to track the version or borrow flag, as they can only change inside
    // an update scope, which guarantees there are never sync issues.
    version: UnsafeCell<u64>,
    borrow: UnsafeCell<BorrowFlag>,
    _marker: marker::PhantomData<Cell<&'store ()>>,
    value: UnsafeCell<T>,
}

impl<'store, T> VersionedCell<'store, T> {
    /// Returns a new [VersionedCell] that contains the given `value`.
    ///
    /// Requires an [UpdateContext] as proof that this is called inside a "store update scope" (see
    /// [Store::update]). The returned [VersionedCell] will be bound to the update scope to which
    /// the [UpdateContext] belongs. It can only outlive that scope be being stored as part of that
    /// store's data graph.
    ///
    /// # Example
    ///
    ///
    #[inline]
    pub fn new(context: UpdateContext<'store>, value: T) -> Self {
        let version = context.next_version();

        VersionedCell {
            version: UnsafeCell::new(version),
            borrow: UnsafeCell::new(UNUSED),
            value: UnsafeCell::new(value),
            _marker: marker::PhantomData,
        }
    }

    #[inline]
    pub fn version(&self) -> u64 {
        unsafe { *self.version.get() }
    }

    #[allow(unused)]
    #[inline]
    pub fn touch(&self, context: UpdateContext<'store>) {
        let new_version = context.next_version();

        // SAFETY: the `UpdateContext` guarantees no other concurrent access.
        unsafe {
            *self.version.get() = new_version;
        }
    }

    /// Returns a reference to the inner value.
    ///
    /// Only available inside a store read-only scope, requires a proof in the form of a
    /// [ReadContext] value (scoped to the same read-only scope as this versioned cell) be passed as
    /// the `context` argument.
    ///
    /// To obtain references to the inner data in an update context, see [borrow] and [borrow_mut],
    /// which do runtime borrow tracking similar to [RefCell] in order to avoid writes to aliased
    /// memory.
    #[allow(unused)]
    #[inline]
    pub fn deref(&self, context: ReadContext<'store>) -> &T {
        // SAFETY: the `ReadContext` guarantees the value cannot be mutably referenced for the
        // lifetime of the reference returned here.
        unsafe { &*self.value.get() }
    }

    #[inline]
    pub fn borrow<'a>(&'a self, context: UpdateContext<'store>) -> Ref<'a, T> {
        self.try_borrow(context).expect("already mutably borrowed")
    }

    #[allow(unused)]
    #[inline]
    pub fn try_borrow<'a>(
        &'a self,
        context: UpdateContext<'store>,
    ) -> Result<Ref<'a, T>, BorrowError> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => {
                // SAFETY: the combination of the `UpdateContext` and `BorrowRef` guarantees unique
                // access.
                Ok(Ref {
                    value: unsafe { &*self.value.get() },
                    borrow: b,
                })
            }
            None => Err(BorrowError {}),
        }
    }

    #[inline]
    pub fn borrow_mut<'a>(&'a self, context: UpdateContext<'store>) -> RefMut<'a, T> {
        self.try_borrow_mut(context).expect("already borrowed")
    }

    #[inline]
    pub fn try_borrow_mut<'a>(
        &'a self,
        context: UpdateContext<'store>,
    ) -> Result<RefMut<'a, T>, BorrowMutError> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => {
                self.touch(context);

                // SAFETY: the combination of the `UpdateContext` and `BorrowRefMut` guarantees
                // unique access.
                Ok(RefMut {
                    value: unsafe { &mut *self.value.get() },
                    borrow: b,
                })
            }
            None => Err(BorrowMutError {}),
        }
    }
}

// SAFETY: all `UnsafeCell`'s inside are only ever written to inside an update scope, which ensures
// writes are synchronized.
unsafe impl<T> Sync for VersionedCell<'_, T> {}

// Modified from `core::cell`.

type BorrowFlag = isize;
const UNUSED: BorrowFlag = 0;

#[inline(always)]
fn is_writing(x: BorrowFlag) -> bool {
    x < UNUSED
}

#[inline(always)]
fn is_reading(x: BorrowFlag) -> bool {
    x > UNUSED
}

// Note that all UnsafeCell dereferencing of the BorrowFlag is only safe because it is guaranteed to
// only happen in an update context, and as such there are no sync issues.

struct BorrowRef<'b> {
    borrow: &'b UnsafeCell<BorrowFlag>,
}

impl<'b> BorrowRef<'b> {
    #[inline]
    fn new(borrow: &'b UnsafeCell<BorrowFlag>) -> Option<BorrowRef<'b>> {
        let ptr = borrow.get();

        let b = unsafe { (*ptr).wrapping_add(1) };

        if !is_reading(b) {
            // Incrementing borrow can result in a non-reading value (<= 0) in these cases:
            // 1. It was < 0, i.e. there are writing borrows, so we can't allow a read borrow
            //    due to Rust's reference aliasing rules
            // 2. It was isize::MAX (the max amount of reading borrows) and it overflowed
            //    into isize::MIN (the max amount of writing borrows) so we can't allow
            //    an additional read borrow because isize can't represent so many read borrows
            //    (this can only happen if you mem::forget more than a small constant amount of
            //    `Ref`s, which is not good practice)
            None
        } else {
            // Incrementing borrow can result in a reading value (> 0) in these cases:
            // 1. It was = 0, i.e. it wasn't borrowed, and we are taking the first read borrow
            // 2. It was > 0 and < isize::MAX, i.e. there were read borrows, and isize
            //    is large enough to represent having one more read borrow
            unsafe {
                *ptr = b;
            }

            Some(BorrowRef { borrow })
        }
    }
}

impl Drop for BorrowRef<'_> {
    #[inline]
    fn drop(&mut self) {
        let ptr = self.borrow.get();

        let borrow = unsafe { *ptr };

        debug_assert!(is_reading(borrow));

        unsafe {
            *ptr = borrow - 1;
        }
    }
}

impl Clone for BorrowRef<'_> {
    #[inline]
    fn clone(&self) -> Self {
        let ptr = self.borrow.get();

        // Since this Ref exists, we know the borrow flag
        // is a reading borrow.
        let borrow = unsafe { *ptr };
        debug_assert!(is_reading(borrow));

        // Prevent the borrow counter from overflowing into
        // a writing borrow.
        assert!(borrow != isize::MAX);
        unsafe {
            *ptr = borrow + 1;
        }

        BorrowRef {
            borrow: self.borrow,
        }
    }
}

pub struct Ref<'b, T: ?Sized + 'b> {
    value: &'b T,
    #[allow(unused)]
    borrow: BorrowRef<'b>,
}

impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Ref<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Ref<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

struct BorrowRefMut<'b> {
    borrow: &'b UnsafeCell<BorrowFlag>,
}

impl<'b> BorrowRefMut<'b> {
    #[inline]
    fn new(borrow: &'b UnsafeCell<BorrowFlag>) -> Option<BorrowRefMut<'b>> {
        let ptr = borrow.get();

        // NOTE: Unlike BorrowRefMut::clone, new is called to create the initial
        // mutable reference, and so there must currently be no existing
        // references. Thus, while clone increments the mutable refcount, here
        // we explicitly only allow going from UNUSED to UNUSED - 1.
        match unsafe { *ptr } {
            UNUSED => {
                unsafe {
                    *ptr = UNUSED - 1;
                }

                Some(BorrowRefMut { borrow })
            }
            _ => None,
        }
    }
}

impl Drop for BorrowRefMut<'_> {
    #[inline]
    fn drop(&mut self) {
        let ptr = self.borrow.get();

        let borrow = unsafe { *ptr };

        debug_assert!(is_writing(borrow));

        unsafe {
            *ptr = borrow + 1;
        }
    }
}

pub struct RefMut<'b, T: ?Sized + 'b> {
    value: &'b mut T,
    #[allow(unused)]
    borrow: BorrowRefMut<'b>,
}

impl<T: ?Sized> Deref for RefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> DerefMut for RefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RefMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RefMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}
