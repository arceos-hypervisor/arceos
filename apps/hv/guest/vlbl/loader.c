#include "defs.h"

#pragma pack(push, 1)
typedef struct _kernel_header {
    uint8_t _01f0;
    uint8_t setup_sects;
    uint16_t root_flags;
    uint32_t syssize;
    uint16_t ramsize;
    uint16_t vid_mode;
    uint16_t root_dev;
    uint16_t boot_flag;
    uint16_t jump;
    uint32_t header;
    uint16_t version;
    uint32_t realmode_swtch;
    uint16_t start_sys_seg;
    uint16_t kernel_version;
    uint8_t type_of_loader;
    uint8_t loadflags;
    uint16_t setup_move_size;
    uint32_t code32_start;
    uint32_t ramdisk_image;
    uint32_t ramdisk_size;
    uint32_t bootsect_kludge;
    uint16_t heap_end_ptr;
    uint8_t ext_loader_ver;
    uint8_t ext_loader_type;
    uint32_t cmd_line_ptr;
    uint32_t initrd_addr_max;
    uint32_t kernel_alignment;
    uint8_t relocatable_kernel;
    uint8_t min_alignment;
    uint16_t xloadflags;
    uint32_t cmdline_size;
    uint32_t hardware_subarch;
    uint32_t hardware_subarch_data_l;
    uint32_t hardware_subarch_data_h;
    uint32_t payload_offset;
    uint32_t payload_length;
    uint32_t setup_data_l;
    uint32_t setup_data_h;
    uint32_t pref_address_l;
    uint32_t pref_address_h;
    uint32_t init_size;
    uint32_t handover_offset;
    uint32_t kernel_info_offset;
} kernel_header, *kernel_header_ptr;
#pragma pack(pop)

void cpy4(void *dst, const void *src, uint32_t size) {
    const uint32_t * ptr_src = src;
    uint32_p ptr_dst = dst;
    for (int i = 0; i < size; i += 4) {
        *ptr_dst = *ptr_src;
        ptr_src++;
        ptr_dst++;
    }
}


const char cmd[256] = "console=uart8250,io,0x3f8,115200n8 debug\0";
// const char cmd[256] = "console=uart8250,io,0x3f8,115200n8 debug\0";

/*
 * load linux kernel image from <void *kernel_image> to <void *loc_real> (for real-mode part) and <void *loc_prot> (for protected-mode part) and fill kernel header
 * 
 * stack_end: end of boot stack
 * 
 * 
 **/
int load_kernel(void *kernel_image, void *loc_real, void *stack_end, void *loc_prot, void *initramfs, uint32_t initramfs_size) {
    puts("[vlbl] loading kernel...");

    kernel_header_ptr orig_header = kernel_image + 0x1f0;

    uint32_t kernel_lower_size = ((orig_header->setup_sects ? orig_header->setup_sects : 4) + 1) * 512;
    uint32_t kernel_upper_size = orig_header->syssize * 16;
    void *prot = kernel_image + kernel_lower_size;

    cpy4(loc_real, kernel_image, kernel_lower_size);
    cpy4(loc_prot, prot, kernel_upper_size);

    void *cmd_base = stack_end;
    void *setup_data_base = cmd_base + 256;

    cpy4(cmd_base, cmd, 256);
    putsi("[vlbl] cmdline: ");
    puts(cmd_base);

    kernel_header_ptr header = loc_real + 0x1f0;

    header->vid_mode = 0xffff;
    header->type_of_loader = 0xff;
    header->loadflags = (header->loadflags & 0x1f) | 0x80;
    header->code32_start = (uint32_t)(loc_prot);
    header->heap_end_ptr = (uint16_t)(stack_end - loc_real - 0x200);
    header->cmd_line_ptr = (uint32_t)cmd_base;
    header->setup_data_l = 0;
    header->setup_data_h = 0;

    if (header->initrd_addr_max < (uint32_t)initramfs + initramfs_size) {
        putsi("[vlbl] cannot load initrd because of a too small initrd_addr_max: ");
        putux(header->initrd_addr_max, true, 8);
        putchar('\n');

        header->ramdisk_image = 0;
        header->ramdisk_size = 0;
    } else {
        header->ramdisk_image = (uint32_t)initramfs;
        header->ramdisk_size = initramfs_size;
    }

    puts("[vlbl] kernel loaded.");

    return 0;
}
