use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            // TODO: proper stack allocation
            const STACK_SIZE: u64 = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE as usize] = [0; STACK_SIZE as usize];

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static::lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        gdt.append(Descriptor::UserSegment(0));
        gdt.append(Descriptor::user_code_segment());
        gdt.append(Descriptor::user_data_segment());

        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (gdt, Selectors{code_selector, data_selector, tss_selector})
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    log::trace!("loading gdt");
    GDT.0.load();
    log::trace!("loaded gdt");

    log::trace!("setting registers for gdt");
    // https://github.com/phil-opp/blog_os/discussions/1005#discussioncomment-7246149
    unsafe {
        use x86_64::instructions::{segmentation, segmentation::Segment, tables};
        segmentation::CS::set_reg(GDT.1.code_selector);
        segmentation::DS::set_reg(GDT.1.data_selector);
        segmentation::ES::set_reg(GDT.1.data_selector);
        segmentation::SS::set_reg(GDT.1.data_selector);
        tables::load_tss(GDT.1.tss_selector);
    }
    log::trace!("setted registers for gdt");
    log::trace!("finished gdt initialization");
}
