//! Memory management for BBC BASIC interpreter
//! 
//! Emulates the exact memory layout of the BBC Model B with 32K RAM,
//! including proper memory mapping and allocation.

use crate::error::{BBCBasicError, Result};

/// BBC Model B memory constants
pub const MEMORY_SIZE: usize = 32768; // 32K RAM
pub const PAGE: u16 = 0x1900;         // Start of user memory
pub const HIMEM: u16 = 0x8000;        // End of user memory
pub const ZERO_PAGE_SIZE: usize = 0x100;
pub const STACK_START: u16 = 0x0100;
pub const STACK_SIZE: usize = 0x100;

/// Memory manager for the BBC BASIC interpreter
#[derive(Debug, Clone)]
pub struct MemoryManager {
    /// The main 32K RAM array
    ram: [u8; MEMORY_SIZE],
    /// Current top of used memory
    top: u16,
    /// Allocation tracking
    allocations: Vec<MemoryAllocation>,
}

/// Represents a memory allocation
#[derive(Debug, Clone)]
struct MemoryAllocation {
    start: u16,
    size: usize,
    allocation_type: AllocationType,
}

/// Types of memory allocations
#[derive(Debug, Clone, PartialEq)]
enum AllocationType {
    Program,
    Variables,
    Stack,
    System,
}

impl MemoryManager {
    /// Create a new memory manager with initialized memory
    pub fn new() -> Self {
        let mut manager = Self {
            ram: [0; MEMORY_SIZE],
            top: PAGE,
            allocations: Vec::new(),
        };
        
        // Initialize system memory areas
        manager.initialize_system_memory();
        manager
    }

    /// Initialize system memory areas
    fn initialize_system_memory(&mut self) {
        // Zero page is already zeroed
        // Stack area is already zeroed
        // System workspace is already zeroed
    }

    /// Read a byte from memory (PEEK operation)
    pub fn peek(&self, address: u16) -> Result<u8> {
        let addr = address as usize;
        if addr >= MEMORY_SIZE {
            return Err(BBCBasicError::InvalidAddress(address));
        }
        Ok(self.ram[addr])
    }

    /// Write a byte to memory (POKE operation)
    pub fn poke(&mut self, address: u16, value: u8) -> Result<()> {
        let addr = address as usize;
        if addr >= MEMORY_SIZE {
            return Err(BBCBasicError::InvalidAddress(address));
        }
        
        // Check if this is a protected system area
        if addr < PAGE as usize && !self.is_safe_system_write(address) {
            // Allow writes to some system areas but be careful
            // For now, allow all writes but this could be restricted
        }
        
        self.ram[addr] = value;
        Ok(())
    }

    /// Check if a system memory write is safe
    fn is_safe_system_write(&self, address: u16) -> bool {
        // For now, allow most system writes
        // In a full implementation, this would check for critical system variables
        address >= 0x0200 // Allow writes above system workspace
    }

    /// Get the PAGE value (start of user memory)
    pub fn get_page(&self) -> u16 {
        PAGE
    }

    /// Get the HIMEM value (end of user memory)
    pub fn get_himem(&self) -> u16 {
        HIMEM
    }

    /// Get the current TOP value (top of used memory)
    pub fn get_top(&self) -> u16 {
        self.top
    }

    /// Allocate memory for program storage
    pub fn allocate_program_space(&mut self, size: usize) -> Result<u16> {
        self.allocate_memory(size, AllocationType::Program)
    }

    /// Allocate memory for variable storage
    pub fn allocate_variable_space(&mut self, size: usize) -> Result<u16> {
        self.allocate_memory(size, AllocationType::Variables)
    }

    /// Generic memory allocation
    fn allocate_memory(&mut self, size: usize, allocation_type: AllocationType) -> Result<u16> {
        let available_space = (HIMEM - self.top) as usize;
        if size > available_space {
            return Err(BBCBasicError::NoRoom);
        }

        let start_address = self.top;
        self.top += size as u16;

        self.allocations.push(MemoryAllocation {
            start: start_address,
            size,
            allocation_type,
        });

        Ok(start_address)
    }

    /// Free all allocations of a specific type
    pub fn free_allocations(&mut self, allocation_type: AllocationType) {
        self.allocations.retain(|alloc| alloc.allocation_type != allocation_type);
        self.recalculate_top();
    }

    /// Recalculate the top of memory after freeing allocations
    fn recalculate_top(&mut self) {
        if self.allocations.is_empty() {
            self.top = PAGE;
        } else {
            self.top = self.allocations
                .iter()
                .map(|alloc| alloc.start + alloc.size as u16)
                .max()
                .unwrap_or(PAGE);
        }
    }

    /// Get available memory
    pub fn get_available_memory(&self) -> usize {
        (HIMEM - self.top) as usize
    }

    /// Clear all user memory
    pub fn clear_user_memory(&mut self) {
        // Clear user memory area
        for addr in PAGE as usize..HIMEM as usize {
            self.ram[addr] = 0;
        }
        
        // Reset allocations and top
        self.allocations.clear();
        self.top = PAGE;
    }

    /// Read a 16-bit word from memory (little-endian)
    pub fn peek_word(&self, address: u16) -> Result<u16> {
        let low = self.peek(address)? as u16;
        let high = self.peek(address + 1)? as u16;
        Ok(low | (high << 8))
    }

    /// Write a 16-bit word to memory (little-endian)
    pub fn poke_word(&mut self, address: u16, value: u16) -> Result<()> {
        self.poke(address, (value & 0xFF) as u8)?;
        self.poke(address + 1, (value >> 8) as u8)?;
        Ok(())
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_constants() {
        assert_eq!(MEMORY_SIZE, 32768);
        assert_eq!(PAGE, 0x1900);
        assert_eq!(HIMEM, 0x8000);
    }

    #[test]
    fn test_memory_manager_creation() {
        let mem = MemoryManager::new();
        assert_eq!(mem.get_page(), PAGE);
        assert_eq!(mem.get_himem(), HIMEM);
        assert_eq!(mem.get_top(), PAGE);
    }

    #[test]
    fn test_peek_poke() {
        let mut mem = MemoryManager::new();
        
        // Test basic peek/poke
        mem.poke(0x2000, 0x42).unwrap();
        assert_eq!(mem.peek(0x2000).unwrap(), 0x42);
        
        // Test invalid address
        assert!(mem.peek(0x8000).is_err());
        assert!(mem.poke(0x8000, 0x42).is_err());
    }

    #[test]
    fn test_word_operations() {
        let mut mem = MemoryManager::new();
        
        mem.poke_word(0x2000, 0x1234).unwrap();
        assert_eq!(mem.peek_word(0x2000).unwrap(), 0x1234);
        
        // Check little-endian storage
        assert_eq!(mem.peek(0x2000).unwrap(), 0x34);
        assert_eq!(mem.peek(0x2001).unwrap(), 0x12);
    }

    #[test]
    fn test_memory_allocation() {
        let mut mem = MemoryManager::new();
        
        let addr1 = mem.allocate_program_space(100).unwrap();
        assert_eq!(addr1, PAGE);
        assert_eq!(mem.get_top(), PAGE + 100);
        
        let addr2 = mem.allocate_variable_space(50).unwrap();
        assert_eq!(addr2, PAGE + 100);
        assert_eq!(mem.get_top(), PAGE + 150);
    }

    #[test]
    fn test_memory_exhaustion() {
        let mut mem = MemoryManager::new();
        
        let available = mem.get_available_memory();
        let result = mem.allocate_program_space(available + 1);
        assert!(matches!(result, Err(BBCBasicError::NoRoom)));
    }
}