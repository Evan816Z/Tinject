use super::super::{InjectContext, InjectError};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualProtectEx, MEM_COMMIT, MEM_RESERVE,
    PAGE_EXECUTE_READ, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, WaitForSingleObject, INFINITE,
};

/// ManualMap 注入
/// 手动将DLL的PE映像写入目标进程内存，不调用LoadLibraryA加载
/// 绕过模块枚举检测，隐蔽性最强；当前实现为简化版，PE映射后仍以LoadLibraryA作为fallback
pub fn inject_with_context(ctx: &InjectContext) -> Result<(), InjectError> {
    let dll_data = &ctx.dll_data;

    // 验证PE头
    if dll_data.len() < 2 || &dll_data[0..2] != b"MZ" {
        return Err(InjectError::ManualMapFailed("无效的PE文件".to_string()));
    }

    unsafe {
        // 解析PE头获取映像大小
        let dos_header = dll_data.as_ptr() as *const IMAGE_DOS_HEADER;
        let nt_headers_offset = (*dos_header).e_lfanew as usize;
        let nt_headers = dll_data.as_ptr().add(nt_headers_offset) as *const IMAGE_NT_HEADERS;
        let image_size = (*nt_headers).optional_header.size_of_image as usize;
        log::debug!("PE 映像大小: {} bytes", image_size);

        // 在目标进程中分配内存
        log::debug!("在目标进程分配 PE 映像内存");
        let remote_base = VirtualAllocEx(
            ctx.process_handle,
            None,
            image_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if remote_base.is_null() {
            log::error!("VirtualAllocEx 分配 PE 映像内存失败");
            return Err(InjectError::VirtualAllocFailed(0));
        }
        log::debug!("PE 映像内存分配成功: remote_base={:?}", remote_base);

        // 写入PE头
        let header_size = (*nt_headers).optional_header.size_of_headers as usize;
        let mut written = 0usize;
        log::debug!("写入 PE 头: {} bytes", header_size);
        let _ = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
            ctx.process_handle,
            remote_base,
            dll_data.as_ptr() as *const _,
            header_size,
            Some(&mut written),
        );

        // 写入各个节
        let section_count = (*nt_headers).file_header.number_of_sections as usize;
        let section_table_offset = nt_headers_offset
            + std::mem::size_of::<IMAGE_NT_HEADERS>();
        log::debug!("写入 {} 个 PE 节", section_count);

        for i in 0..section_count {
            let section_header = dll_data.as_ptr().add(section_table_offset + i * 40)
                as *const IMAGE_SECTION_HEADER;
            let section_va = remote_base as usize + (*section_header).virtual_address as usize;
            let section_size = (*section_header).size_of_raw_data as usize;
            let section_ptr = (*section_header).pointer_to_raw_data as usize;

            if section_size > 0 && section_ptr > 0 && section_ptr + section_size <= dll_data.len() {
                log::debug!("写入节 #{}: va=0x{:x}, size={}", i, section_va, section_size);
                let _ = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                    ctx.process_handle,
                    section_va as *mut _,
                    dll_data.as_ptr().add(section_ptr) as *const _,
                    section_size,
                    Some(&mut written),
                );
            } else {
                log::debug!("跳过节 #{}: size={}", i, section_size);
            }
        }

        // 设置内存页保护属性
        log::debug!("修改 PE 映像内存保护为 EXECUTE_READ");
        let _ = VirtualProtectEx(
            ctx.process_handle,
            remote_base,
            image_size,
            PAGE_EXECUTE_READ,
            std::ptr::null_mut(),
        );

        // 简化版：使用CreateRemoteThread + LoadLibraryA作为fallback
        log::info!("ManualMap fallback: 创建远程线程调用 LoadLibraryA");
        let thread = CreateRemoteThread(
            ctx.process_handle,
            None,
            0,
            Some(std::mem::transmute(ctx.load_library_addr)),
            Some(ctx.remote_dll_path),
            0,
            None,
        )
        .map_err(|e| {
            log::error!("CreateRemoteThread 失败: {}", e);
            InjectError::ManualMapFailed("创建远程线程失败".to_string())
        })?;

        log::debug!("等待远程线程完成...");
        let _ = WaitForSingleObject(thread, INFINITE);
        log::info!("远程线程执行完成");
        let _ = CloseHandle(thread);
    }

    Ok(())
}

// PE结构体定义
#[repr(C)]
struct IMAGE_DOS_HEADER {
    e_magic: u16,
    _e_cblp: u16,
    _e_cp: u16,
    _e_crlc: u16,
    _e_cparhdr: u16,
    _e_minalloc: u16,
    _e_maxalloc: u16,
    _e_ss: u16,
    _e_sp: u16,
    _e_csum: u16,
    _e_ip: u16,
    _e_cs: u16,
    _e_lfarlc: u16,
    _e_ovno: u16,
    _e_res: [u16; 4],
    _e_oemid: u16,
    _e_oeminfo: u16,
    _e_res2: [u16; 10],
    e_lfanew: i32,
}

#[repr(C)]
struct IMAGE_NT_HEADERS {
    _signature: u32,
    file_header: IMAGE_FILE_HEADER,
    optional_header: IMAGE_OPTIONAL_HEADER,
}

#[repr(C)]
struct IMAGE_FILE_HEADER {
    _machine: u16,
    number_of_sections: u16,
    _time_date_stamp: u32,
    _pointer_to_symbol_table: u32,
    _number_of_symbols: u32,
    _size_of_optional_header: u16,
    _characteristics: u16,
}

#[repr(C)]
struct IMAGE_OPTIONAL_HEADER {
    _magic: u16,
    _major_linker_version: u8,
    _minor_linker_version: u8,
    _size_of_code: u32,
    _size_of_initialized_data: u32,
    _size_of_uninitialized_data: u32,
    _address_of_entry_point: u32,
    _base_of_code: u32,
    _image_base: u64,
    _section_alignment: u32,
    _file_alignment: u32,
    _major_os_version: u16,
    _minor_os_version: u16,
    _major_image_version: u16,
    _minor_image_version: u16,
    _major_subsystem_version: u16,
    _minor_subsystem_version: u16,
    _win32_version_value: u32,
    size_of_image: u32,
    size_of_headers: u32,
    _checksum: u32,
    _subsystem: u16,
    _dll_characteristics: u16,
    _size_of_stack_reserve: u64,
    _size_of_stack_commit: u64,
    _size_of_heap_reserve: u64,
    _size_of_heap_commit: u64,
    _loader_flags: u32,
    _number_of_rva_and_sizes: u32,
}

#[repr(C)]
struct IMAGE_SECTION_HEADER {
    _name: [u8; 8],
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    _pointer_to_relocations: u32,
    _pointer_to_linenumbers: u32,
    _number_of_relocations: u16,
    _number_of_linenumbers: u16,
    _characteristics: u32,
}
