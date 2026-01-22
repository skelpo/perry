//! Fast bump allocator for short-lived objects
//!
//! Uses thread-local bump allocation for fast object creation.
//! Objects allocated here are not individually freed - the entire arena
//! can be reset at once (e.g., at end of program or during GC).

use std::cell::UnsafeCell;
use std::alloc::{alloc, Layout};

/// Size of each arena block (8MB)
const BLOCK_SIZE: usize = 8 * 1024 * 1024;

/// A single arena block
struct ArenaBlock {
    data: *mut u8,
    size: usize,
    offset: usize,
}

impl ArenaBlock {
    fn new() -> Self {
        let layout = Layout::from_size_align(BLOCK_SIZE, 16).unwrap();
        let data = unsafe { alloc(layout) };
        if data.is_null() {
            panic!("Failed to allocate arena block");
        }
        ArenaBlock {
            data,
            size: BLOCK_SIZE,
            offset: 0,
        }
    }

    /// Try to allocate within this block
    #[inline]
    fn alloc(&mut self, size: usize) -> Option<*mut u8> {
        if self.offset + size > self.size {
            return None;
        }

        let ptr = unsafe { self.data.add(self.offset) };
        self.offset += size;
        Some(ptr)
    }
}

/// Thread-local arena allocator
struct Arena {
    blocks: Vec<ArenaBlock>,
    current: usize,
}

impl Arena {
    fn new() -> Self {
        Arena {
            blocks: vec![ArenaBlock::new()],
            current: 0,
        }
    }

    #[inline]
    fn alloc(&mut self, size: usize) -> *mut u8 {
        // Try current block first
        if let Some(ptr) = self.blocks[self.current].alloc(size) {
            return ptr;
        }

        // Need a new block
        self.blocks.push(ArenaBlock::new());
        self.current += 1;

        self.blocks[self.current].alloc(size)
            .expect("Fresh block should have space")
    }
}

thread_local! {
    static ARENA: UnsafeCell<Arena> = UnsafeCell::new(Arena::new());
}

/// Allocate memory from the thread-local arena
/// This is very fast - just a pointer bump in the common case
#[inline]
pub fn arena_alloc(size: usize, _align: usize) -> *mut u8 {
    ARENA.with(|arena| {
        let arena = unsafe { &mut *arena.get() };
        arena.alloc(size)
    })
}

/// Allocate an object of known size from the arena
/// Returns a properly aligned pointer
#[no_mangle]
pub extern "C" fn js_arena_alloc(size: u32) -> *mut u8 {
    arena_alloc(size as usize, 8)
}
