// Export one of the malloc libraries.

#[cfg(feature = "malloc_jemalloc")]
pub use self::jemalloc::*;
#[cfg(all(
    not(target_os = "windows"),
    not(any(feature = "malloc_jemalloc", feature = "malloc_mimalloc"))
))]
pub use self::libc_malloc::*;
#[cfg(feature = "malloc_mimalloc")]
pub use self::mimalloc::*;
#[cfg(all(
    target_os = "windows",
    not(any(feature = "malloc_jemalloc", feature = "malloc_mimalloc"))
))]
pub use self::win_malloc::*;

/// When we count page usage of library malloc, we assume they allocate in pages. For some malloc implementations,
/// they may use a larger page (e.g. mimalloc's 64K page). For libraries that we are not sure, we assume they use
/// normal 4k pages.
pub const BYTES_IN_MALLOC_PAGE: usize = 1 << LOG_BYTES_IN_MALLOC_PAGE;

// Different malloc libraries

// TODO: We should conditinally include some methods in the module, such as posix extension and GNU extension.

#[cfg(feature = "malloc_jemalloc")]
mod jemalloc {
    // Normal 4K page
    pub const LOG_BYTES_IN_MALLOC_PAGE: u8 = crate::util::constants::LOG_BYTES_IN_PAGE;
    // ANSI C
    pub use jemalloc_sys::{calloc, free, malloc, realloc};
    // Posix
    pub use jemalloc_sys::posix_memalign;
    // GNU
    pub use jemalloc_sys::malloc_usable_size;
}

#[cfg(feature = "malloc_mimalloc")]
mod mimalloc {
    // Normal 4K page accounting
    pub const LOG_BYTES_IN_MALLOC_PAGE: u8 = crate::util::constants::LOG_BYTES_IN_PAGE;
    // ANSI C
    pub use mimalloc_sys::{
        mi_calloc as calloc, mi_free as free, mi_malloc as malloc, mi_realloc as realloc,
    };
    // Posix
    pub use mimalloc_sys::mi_posix_memalign as posix_memalign;
    // GNU
    pub use mimalloc_sys::mi_malloc_usable_size as malloc_usable_size;
}

/// If no malloc lib is specified, use the libc implementation
#[cfg(all(
    not(target_os = "windows"),
    not(any(feature = "malloc_jemalloc", feature = "malloc_mimalloc"))
))]
mod libc_malloc {
    // Normal 4K page
    pub const LOG_BYTES_IN_MALLOC_PAGE: u8 = crate::util::constants::LOG_BYTES_IN_PAGE;
    // ANSI C
    pub use libc::{calloc, free, malloc, realloc};
    // Posix
    pub use libc::posix_memalign;
    // GNU
    #[cfg(target_os = "linux")]
    pub use libc::malloc_usable_size;
    #[cfg(target_os = "macos")]
    extern "C" {
        pub fn malloc_size(ptr: *const libc::c_void) -> usize;
    }
    #[cfg(target_os = "macos")]
    pub use self::malloc_size as malloc_usable_size;
}

/// Windows malloc implementation using HeapAlloc
#[cfg(target_os = "windows")]
mod win_malloc {
    // Normal 4K page
    pub const LOG_BYTES_IN_MALLOC_PAGE: u8 = crate::util::constants::LOG_BYTES_IN_PAGE;

    use std::ffi::c_void;
    use windows_sys::Win32::System::Memory::*;

    // All allocations must be 16-byte aligned on Windows for SSE instructions.
    const MALLOC_ALIGNMENT: usize = 16;

    pub unsafe fn malloc(size: usize) -> *mut c_void {
        let mut ptr = std::ptr::null_mut();
        posix_memalign(&mut ptr, MALLOC_ALIGNMENT, size);
        ptr
    }

    pub unsafe fn free(ptr: *mut c_void) {
        if !ptr.is_null() {
            let original_ptr = *(ptr as *mut *mut c_void).offset(-1);
            HeapFree(GetProcessHeap(), 0, original_ptr);
        }
    }

    pub unsafe fn calloc(nmemb: usize, size: usize) -> *mut c_void {
        let total = nmemb * size;
        let ptr = malloc(total);
        if !ptr.is_null() {
            std::ptr::write_bytes(ptr, 0, total);
        }
        ptr
    }

    pub unsafe fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
        if ptr.is_null() {
            return malloc(size);
        }
        let original_ptr = *(ptr as *mut *mut c_void).offset(-1);
        let new_ptr = HeapReAlloc(GetProcessHeap(), 0, original_ptr, size + MALLOC_ALIGNMENT);
        if new_ptr.is_null() {
            return std::ptr::null_mut();
        }
        let aligned_ptr = (new_ptr as usize + MALLOC_ALIGNMENT - 1) & !(MALLOC_ALIGNMENT - 1);
        *(aligned_ptr as *mut *mut c_void).offset(-1) = new_ptr;
        aligned_ptr as *mut c_void
    }

    pub unsafe fn posix_memalign(memptr: *mut *mut c_void, alignment: usize, size: usize) -> i32 {
        let total_size = size + alignment + std::mem::size_of::<*mut c_void>();
        let original_ptr = HeapAlloc(GetProcessHeap(), 0, total_size);
        if original_ptr.is_null() {
            return 12; // ENOMEM
        }
        let aligned_ptr = (original_ptr as usize + alignment + std::mem::size_of::<*mut c_void>() - 1) & !(alignment - 1);
        *(aligned_ptr as *mut *mut c_void).offset(-1) = original_ptr;
        *memptr = aligned_ptr as *mut c_void;
        0
    }

    pub unsafe fn malloc_usable_size(ptr: *const c_void) -> usize {
        if ptr.is_null() {
            return 0;
        }
        let original_ptr = *(ptr as *mut *const c_void).offset(-1);
        HeapSize(GetProcessHeap(), 0, original_ptr)
    }
}
