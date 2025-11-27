use super::{Locked, align_up};
use alloc::alloc::{GlobalAlloc, Layout};
use core::{mem, ptr};

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // verifica que a região liberada é capaz de armazenar um ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;

        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr);
        }
    }

    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // região pequena
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;

        // a alocação divide a região em uma parte usada e livre
        // portanto a parte excedente deve conseguir armazenar um ListNode
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    /// ajusta o layout de forma que a região alocada de memória
    /// possa armazenar um ListNode.
    /// alguma hora, as regiões serão desalocadas portanto devem serem capazes de armazenarem
    /// ListNode sem causar UB.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>()) // aumenta o alinhamento para o alinhamento de um ListNode
            .expect("adjusting alignment failed")
            // arredonda o tamanho para um múltiplo do alinhamento para garantir que o próximo bloco de memória vai ter
            // o alinhamento correto para armazenar um ListNode
            .pad_to_align();

        // garante um tamanho mínimo de mem::size_of::<ListNode>
        // dessa maneira, o dealloc pode escrever de forma segura um ListNode para o bloco de memória liberado
        let size = layout.size().max(mem::size_of::<ListNode>());

        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;

            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }

            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
