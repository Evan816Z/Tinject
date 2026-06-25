/**
 * Tinject - 前端交互逻辑 (wry 版本)
 */

// DLL文件列表
let dllFiles = [];

// 进程列表
let processList = [
    { name: 'javaw.exe', selected: true, running: false }
];

// 侧边栏长按液态玻璃效果
let pressTimer = null;
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('mousedown', () => {
        pressTimer = setTimeout(() => {
            item.classList.add('pressed');
        }, 200);
    });

    item.addEventListener('mouseup', () => {
        clearTimeout(pressTimer);
        setTimeout(() => item.classList.remove('pressed'), 300);
    });

    item.addEventListener('mouseleave', () => {
        clearTimeout(pressTimer);
        item.classList.remove('pressed');
    });
});

// 页面导航
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', () => {
        const page = item.dataset.page;

        // 更新导航状态
        document.querySelectorAll('.nav-item').forEach(i => i.classList.remove('active'));
        item.classList.add('active');

        // 切换页面
        document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
        document.getElementById(`page-${page}`).classList.add('active');
    });
});

// 调用后端命令
function invoke(cmd, data = {}) {
    return new Promise((resolve, reject) => {
        window.__callback = (responseCmd, result) => {
            if (responseCmd === cmd) {
                resolve(result);
            }
        };
        
        const message = {
            cmd: cmd,
            data: data
        };
        
        // wry IPC
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify(message));
        } else {
            // 开发模式模拟
            console.log('IPC not available, simulating...');
            setTimeout(() => {
                resolve({ success: true, message: '模拟响应' });
            }, 500);
        }
    });
}

// 添加进程
function addProcess() {
    const input = document.getElementById('newProcessName');
    const name = input.value.trim();
    
    if (!name) {
        addLog('warning', '请输入进程名称');
        return;
    }
    
    if (processList.find(p => p.name === name)) {
        addLog('warning', `进程 ${name} 已存在`);
        return;
    }
    
    processList.push({
        name: name,
        selected: true,
        running: false
    });

    input.value = '';
    renderProcessList();
    refreshProcessStatus();
    persistState();
    addLog('info', `已添加进程: ${name}`);
}

// 移除进程
function removeProcess(index) {
    const name = processList[index].name;
    processList.splice(index, 1);
    renderProcessList();
    persistState();
    addLog('info', `已移除进程: ${name}`);
}

// 切换进程选择状态
function toggleProcess(index) {
    processList[index].selected = !processList[index].selected;
    renderProcessList();
    persistState();
}

// 渲染进程列表
function renderProcessList() {
    const list = document.getElementById('processList');

    if (processList.length === 0) {
        list.innerHTML = '<div class="process-item empty"><span>请添加进程</span></div>';
        return;
    }

    list.innerHTML = processList.map((proc, index) => `
        <div class="process-item ${proc.selected ? 'selected' : ''} ${proc.running ? 'running' : 'stopped'}" data-index="${index}">
            <input type="checkbox" class="process-item-checkbox"
                   ${proc.selected ? 'checked' : ''}
                   onchange="toggleProcess(${index})">
            <span class="process-item-name">${proc.name}</span>
            <span class="process-item-status ${proc.running ? 'running' : ''}">
                ${proc.running ? '运行中' : '未启动'}
            </span>
            <button class="process-item-remove" onclick="removeProcess(${index})">×</button>
        </div>
    `).join('');
}

// 实时刷新进程运行状态（轻量更新，不重建整个列表）
async function refreshProcessStatus() {
    if (processList.length === 0 || typeof window.ipc === 'undefined') return;

    try {
        const names = processList.map(p => p.name);
        const response = await invoke('check_processes_running', names);
        if (!response.success || !response.statuses) return;

        const statusMap = new Map(response.statuses);
        let changed = false;

        processList.forEach((proc, index) => {
            const isRunning = statusMap.get(proc.name) || false;
            if (proc.running !== isRunning) {
                proc.running = isRunning;
                changed = true;

                // 仅更新对应 DOM 项的状态，避免整表重绘
                const item = document.querySelector(`.process-item[data-index="${index}"]`);
                const statusEl = item?.querySelector('.process-item-status');
                if (item) {
                    item.classList.toggle('running', isRunning);
                    item.classList.toggle('stopped', !isRunning);
                }
                if (statusEl) {
                    statusEl.classList.toggle('running', isRunning);
                    statusEl.textContent = isRunning ? '运行中' : '未启动';
                }
            }
        });

        if (changed) {
            persistState();
        }
    } catch (err) {
        // 静默失败，避免日志刷屏
        console.debug('刷新进程状态失败:', err);
    }
}

// 启动实时状态刷新定时器
let processStatusTimer = null;
function startProcessStatusRefresh() {
    if (processStatusTimer) return;
    processStatusTimer = setInterval(refreshProcessStatus, 1500);
}

function stopProcessStatusRefresh() {
    if (processStatusTimer) {
        clearInterval(processStatusTimer);
        processStatusTimer = null;
    }
}

// 系统进程选择器
let availableProcesses = [];
let selectedPickerProcesses = new Set();

async function openProcessPicker() {
    selectedPickerProcesses.clear();
    document.getElementById('processPickerSearch').value = '';
    document.getElementById('processPickerOverlay').classList.add('show');
    document.getElementById('processPickerList').innerHTML = '<div class="process-picker-loading">正在枚举进程...</div>';

    try {
        const response = await invoke('list_processes');
        if (response.success && response.processes) {
            availableProcesses = response.processes;
            renderProcessPicker();
        } else {
            document.getElementById('processPickerList').innerHTML = '<div class="process-picker-empty">无法获取进程列表</div>';
        }
    } catch (err) {
        document.getElementById('processPickerList').innerHTML = `<div class="process-picker-empty">获取进程列表失败: ${err}</div>`;
    }
}

function closeProcessPicker(event) {
    if (event && event.target !== document.getElementById('processPickerOverlay')) return;
    document.getElementById('processPickerOverlay').classList.remove('show');
}

function filterProcessPicker() {
    renderProcessPicker();
}

function renderProcessPicker() {
    const list = document.getElementById('processPickerList');
    const keyword = document.getElementById('processPickerSearch').value.toLowerCase();

    const filtered = availableProcesses.filter(p =>
        p.name.toLowerCase().includes(keyword) ||
        p.path.toLowerCase().includes(keyword)
    );

    if (filtered.length === 0) {
        list.innerHTML = '<div class="process-picker-empty">未找到匹配的进程</div>';
        return;
    }

    list.innerHTML = filtered.map(p => {
        const isSelected = selectedPickerProcesses.has(p.name);
        return `
            <div class="process-picker-item ${isSelected ? 'selected' : ''}" onclick="togglePickerProcess('${p.name.replace(/'/g, "\\'")}')">
                <input type="checkbox" class="process-picker-checkbox" ${isSelected ? 'checked' : ''} onclick="event.stopPropagation()">
                <div class="process-picker-info">
                    <div class="process-picker-name">${p.name}</div>
                    <div class="process-picker-path">PID: ${p.pid} · ${p.path}</div>
                </div>
            </div>
        `;
    }).join('');
}

function togglePickerProcess(name) {
    if (selectedPickerProcesses.has(name)) {
        selectedPickerProcesses.delete(name);
    } else {
        selectedPickerProcesses.add(name);
    }
    renderProcessPicker();
}

function confirmProcessSelection() {
    let added = 0;
    selectedPickerProcesses.forEach(name => {
        if (!processList.find(p => p.name === name)) {
            processList.push({ name, selected: true, running: false });
            added++;
        }
    });

    if (added > 0) {
        renderProcessList();
        startProcessStatusRefresh();
        refreshProcessStatus();
        persistState();
        addLog('info', `已从运行中添加 ${added} 个进程`);
    }

    closeProcessPicker();
}

// 添加DLL文件
async function addDllFile() {
    const result = await invoke('select_file', { filter: 'dll' });

    if (result.success && result.path) {
        const path = result.path;
        const fileName = path.split('\\').pop() || path.split('/').pop();
        dllFiles.push({
            path: path.trim(),
            name: fileName
        });
        appendDllItem(dllFiles.length - 1);
        addLog('info', `已添加 DLL: ${fileName}`);
    }
}

// 新增单个 DLL 项（避免全量重绘）
function appendDllItem(index) {
    const list = document.getElementById('dllList');
    if (dllFiles.length === 1) {
        list.innerHTML = '';
    }
    list.appendChild(createDllElement(dllFiles[index], index));
    persistState();
}

// 创建 DLL DOM 元素
function createDllElement(file, index) {
    const el = document.createElement('div');
    el.className = 'dll-item';
    el.draggable = true;
    el.dataset.index = index;
    el.innerHTML = `
        <div class="dll-item-info">
            <div class="dll-item-drag" title="拖动排序">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                    <circle cx="9" cy="6" r="2" fill="currentColor"/>
                    <circle cx="15" cy="6" r="2" fill="currentColor"/>
                    <circle cx="9" cy="12" r="2" fill="currentColor"/>
                    <circle cx="15" cy="12" r="2" fill="currentColor"/>
                    <circle cx="9" cy="18" r="2" fill="currentColor"/>
                    <circle cx="15" cy="18" r="2" fill="currentColor"/>
                </svg>
            </div>
            <div class="dll-item-icon">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                    <path d="M14 2H6C5.46957 2 4.96086 2.21071 4.58579 2.58579C4.21071 2.96086 4 3.46957 4 4V20C4 20.5304 4.21071 21.0391 4.58579 21.4142C4.96086 21.7893 5.46957 22 6 22H18C18.5304 22 19.0391 21.7893 19.4142 21.4142C19.7893 21.0391 20 20.5304 20 20V8L14 2Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M14 2V8H20" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
            </div>
            <div>
                <div class="dll-item-name">${file.name}</div>
                <div class="dll-item-path">${file.path}</div>
            </div>
        </div>
        <button class="dll-item-remove" onclick="removeDll(${index})">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                <path d="M18 6L6 18M6 6L18 18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
        </button>
    `;

    el.addEventListener('dragstart', handleDllDragStart);
    el.addEventListener('dragover', handleDllDragOver);
    el.addEventListener('drop', handleDllDrop);
    el.addEventListener('dragend', handleDllDragEnd);
    return el;
}

// 渲染DLL列表
function renderDllList() {
    const list = document.getElementById('dllList');

    if (dllFiles.length === 0) {
        list.innerHTML = '<div class="dll-item empty"><span>点击添加 DLL 文件</span></div>';
        return;
    }

    list.innerHTML = '';
    const fragment = document.createDocumentFragment();
    dllFiles.forEach((file, index) => {
        fragment.appendChild(createDllElement(file, index));
    });
    list.appendChild(fragment);
}

// 移除DLL
function removeDll(index) {
    dllFiles.splice(index, 1);
    renderDllList();
    persistState();
    addLog('info', '已移除DLL文件');
}

// DLL 拖拽排序
let dllDragSrcIndex = null;
let dllDragSrcEl = null;

function handleDllDragStart(e) {
    dllDragSrcEl = this;
    dllDragSrcIndex = parseInt(this.dataset.index, 10);
    this.classList.add('dragging');
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', dllDragSrcIndex);
}

function handleDllDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';

    const target = e.currentTarget;
    if (target === dllDragSrcEl) return;

    const targetIndex = parseInt(target.dataset.index, 10);
    const rect = target.getBoundingClientRect();
    const midY = rect.top + rect.height / 2;

    target.classList.toggle('drag-over-top', e.clientY < midY);
    target.classList.toggle('drag-over-bottom', e.clientY >= midY);
}

function handleDllDrop(e) {
    e.preventDefault();
    const target = e.currentTarget;
    if (target === dllDragSrcEl) return;

    const targetIndex = parseInt(target.dataset.index, 10);
    const rect = target.getBoundingClientRect();
    const insertAfter = e.clientY >= rect.top + rect.height / 2;

    // 从原位置移除
    const [moved] = dllFiles.splice(dllDragSrcIndex, 1);
    // 计算新位置
    let newIndex = targetIndex;
    if (dllDragSrcIndex < targetIndex && insertAfter) newIndex = targetIndex;
    if (dllDragSrcIndex < targetIndex && !insertAfter) newIndex = targetIndex - 1;
    if (dllDragSrcIndex > targetIndex && insertAfter) newIndex = targetIndex + 1;
    if (dllDragSrcIndex > targetIndex && !insertAfter) newIndex = targetIndex;

    dllFiles.splice(newIndex, 0, moved);
    renderDllList();
    persistState();
}

function handleDllDragEnd() {
    document.querySelectorAll('.dll-item').forEach(item => {
        item.classList.remove('dragging', 'drag-over-top', 'drag-over-bottom');
    });
    dllDragSrcEl = null;
    dllDragSrcIndex = null;
}

// 开始注入
async function startInjection() {
    if (dllFiles.length === 0) {
        addLog('error', '请先添加DLL文件');
        return;
    }
    
    const selectedProcesses = processList.filter(p => p.selected);
    if (selectedProcesses.length === 0) {
        addLog('error', '请选择至少一个目标进程');
        return;
    }
    
    const btn = document.getElementById('injectBtn');
    btn.disabled = true;
    btn.innerHTML = `
        <svg class="loading" width="20" height="20" viewBox="0 0 24 24" fill="none">
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" stroke-dasharray="31.4 31.4" stroke-dashoffset="0"/>
        </svg>
        注入中...
    `;
    
    addLog('info', `开始批量注入流程，目标进程: ${selectedProcesses.map(p => p.name).join(', ')}`);
    
    let totalSuccess = 0;
    let totalFailed = 0;
    
    try {
        for (const proc of selectedProcesses) {
            addLog('info', `正在处理进程: ${proc.name}`);
            
            const request = {
                dll_paths: dllFiles.map(f => f.path),
                method: document.getElementById('injectMethod').value,
                batch_delay_ms: parseInt(document.getElementById('batchDelay').value) || 500,
                target_processes: [proc.name]
            };
            
            const response = await invoke('inject', request);
            
            if (response.results) {
                response.results.forEach(result => {
                    if (result.success) {
                        totalSuccess++;
                        addLog('success', `[${proc.name}] [${result.method}] ${result.dll_path.split('\\').pop()} - 注入成功`);
                    } else {
                        totalFailed++;
                        addLog('error', `[${proc.name}] ${result.dll_path.split('\\').pop()} - ${result.message}`);
                    }
                });
            }
            
            addLog(response.success ? 'success' : 'warning', `[${proc.name}] ${response.message}`);
        }
        
        addLog('success', `批量注入完成: ${totalSuccess} 成功, ${totalFailed} 失败`);
    } catch (err) {
        addLog('error', `注入失败: ${err}`);
    } finally {
        btn.disabled = false;
        btn.innerHTML = `
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                <path d="M5 12H19M19 12L12 5M19 12L12 19" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            开始注入
        `;
    }
}

// 添加日志
function addLog(level, message) {
    const logContent = document.getElementById('logContent');
    const now = new Date();
    const time = now.toTimeString().split(' ')[0];
    
    const entry = document.createElement('div');
    entry.className = `log-entry ${level}`;
    entry.innerHTML = `
        <span class="log-time">[${time}]</span>
        <span class="log-msg">${message}</span>
    `;
    
    logContent.appendChild(entry);
    logContent.scrollTop = logContent.scrollHeight;
}

// 清空日志
function clearLog() {
    document.getElementById('logContent').innerHTML = '';
    addLog('info', '日志已清空');
}

// 切换注入方式说明面板
function toggleMethodInfo() {
    document.getElementById('methodInfo').classList.toggle('show');
}

// 主题切换
function selectTheme(theme) {
    document.querySelectorAll('.theme-option').forEach(opt => {
        opt.classList.toggle('active', opt.dataset.theme === theme);
    });
    
    if (theme === 'glass') {
        document.documentElement.removeAttribute('data-theme');
    } else {
        document.documentElement.setAttribute('data-theme', theme);
    }
    
    addLog('info', `已切换到 ${theme} 主题`);
    saveSettings();
}

// 更新透明度
function updateOpacity(value) {
    document.getElementById('opacityValue').textContent = value + '%';
    document.documentElement.style.setProperty('--app-opacity', value / 100);
    saveSettings();
}

// 更新模糊强度
function updateBlur(value) {
    document.getElementById('blurValue').textContent = value + 'px';
    document.documentElement.style.setProperty('--blur-intensity', value + 'px');
    saveSettings();
}

// 更新主色调
function updateAccentColor(color) {
    document.getElementById('accentColor').value = color;
    document.documentElement.style.setProperty('--accent', color);
    
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);
    document.documentElement.style.setProperty('--accent-glow', `rgba(${r}, ${g}, ${b}, 0.3)`);
    
    saveSettings();
}

// 选择背景图片
async function selectBgImage() {
    const result = await invoke('select_file', { filter: 'image' });
    if (result.success && result.path) {
        setBgImage(result.path);
    }
}

// 设置背景图片
async function setBgImage(path) {
    const bgLayer = document.getElementById('bgLayer');
    const bgDefault = document.getElementById('bgDefault');

    const result = await invoke('read_image_base64', { path });
    if (result.success && result.data) {
        bgLayer.style.backgroundImage = `url('${result.data}')`;
        bgDefault.style.display = 'none';

        const filename = document.getElementById('bgFilename');
        filename.textContent = path.split(/[/\\]/).pop();
        filename.dataset.path = path;
        saveSettings();
    } else {
        addLog('error', `背景图片加载失败: ${result.message || '未知错误'}`);
    }
}

// 清除背景图片
function clearBgImage() {
    const bgLayer = document.getElementById('bgLayer');
    const bgDefault = document.getElementById('bgDefault');
    
    bgLayer.style.backgroundImage = '';
    bgDefault.style.display = 'block';
    
    document.getElementById('bgFilename').textContent = '未设置';
    saveSettings();
}

// 保存设置
function saveSettings() {
    const settings = {
        theme: document.querySelector('.theme-option.active')?.dataset.theme || 'glass',
        opacity: parseInt(document.getElementById('opacitySlider').value),
        blur: parseInt(document.getElementById('blurSlider').value),
        accentColor: document.getElementById('accentColor').value,
        bgImage: document.getElementById('bgFilename').textContent === '未设置' ? '' : document.getElementById('bgFilename').dataset.path || '',
        processes: processList
    };

    localStorage.setItem('tinject_settings', JSON.stringify(settings));
}

// 加载设置
function loadSettings() {
    const saved = localStorage.getItem('tinject_settings');
    if (saved) {
        const settings = JSON.parse(saved);

        if (settings.theme) {
            selectTheme(settings.theme);
        }
        if (settings.opacity) {
            document.getElementById('opacitySlider').value = settings.opacity;
            updateOpacity(settings.opacity);
        }
        if (settings.blur) {
            document.getElementById('blurSlider').value = settings.blur;
            updateBlur(settings.blur);
        }
        if (settings.accentColor) {
            updateAccentColor(settings.accentColor);
        }
        if (settings.bgImage) {
            setBgImage(settings.bgImage);
        }
        if (settings.processes && Array.isArray(settings.processes)) {
            // 加载时重置运行状态，由实时刷新机制检测真实状态
            processList = settings.processes.map(p => ({
                name: p.name,
                selected: p.selected,
                running: false
            }));
        }
    }

    renderProcessList();
}

// 保存配置（自动持久化 DLL 和进程选择）
async function saveConfig() {
    const config = {
        injection: {
            target_processes: processList.map(p => p.name),
            persisted_processes: processList.map(p => ({ name: p.name, selected: p.selected })),
            method: document.getElementById('cfgInjectMethod').value,
            process_timeout_ms: parseInt(document.getElementById('cfgTimeout').value),
            batch_delay_ms: parseInt(document.getElementById('cfgBatchDelay').value),
            auto_fallback: document.getElementById('cfgAutoFallback').checked,
            dll_paths: dllFiles.map(f => f.path)
        },
        ui: {
            theme: document.querySelector('.theme-option.active')?.dataset.theme || 'glass',
            opacity: parseInt(document.getElementById('opacitySlider').value) / 100,
            blur_intensity: parseInt(document.getElementById('blurSlider').value),
            accent_color: document.getElementById('accentColor').value,
            window_width: 900,
            window_height: 620
        }
    };

    try {
        const result = await invoke('save_config', config);
        if (result.success) {
            logPersistence('配置已保存');
        } else {
            addLog('error', `保存配置失败: ${result.message}`);
        }
    } catch (err) {
        addLog('error', `保存配置失败: ${err}`);
    }
}

// 加载配置（恢复 DLL 和进程选择）
async function loadConfig() {
    try {
        const config = await invoke('get_config');

        // 恢复注入方式与延迟
        if (config.injection) {
            document.getElementById('cfgInjectMethod').value = config.injection.method || 'auto';
            document.getElementById('cfgTimeout').value = config.injection.process_timeout_ms || 30000;
            document.getElementById('cfgBatchDelay').value = config.injection.batch_delay_ms || 500;
            document.getElementById('cfgAutoFallback').checked = config.injection.auto_fallback !== false;

            document.getElementById('injectMethod').value = config.injection.method || 'auto';
            document.getElementById('batchDelay').value = config.injection.batch_delay_ms || 500;

            // 恢复进程选择
            if (config.injection.persisted_processes && config.injection.persisted_processes.length > 0) {
                processList = config.injection.persisted_processes.map(p => ({
                    name: p.name,
                    selected: p.selected,
                    running: false
                }));
            } else if (config.injection.target_processes && config.injection.target_processes.length > 0) {
                processList = config.injection.target_processes.map(name => ({
                    name,
                    selected: true,
                    running: false
                }));
            }

            // 恢复 DLL 列表
            if (config.injection.dll_paths && config.injection.dll_paths.length > 0) {
                dllFiles = config.injection.dll_paths.map(path => ({
                    path,
                    name: path.split('\\').pop() || path.split('/').pop() || path
                }));
            }
        }

        renderProcessList();
        renderDllList();
        refreshProcessStatus();
    } catch (err) {
        console.error('加载配置失败:', err);
    }
}

// 静默持久化辅助函数（避免频繁在日志面板刷屏）
let persistenceDebounceTimer = null;
function persistState() {
    clearTimeout(persistenceDebounceTimer);
    persistenceDebounceTimer = setTimeout(() => {
        saveConfig();
    }, 300);
}

function logPersistence(message) {
    // 仅在开发模式或需要时输出；默认不在主日志面板显示，避免干扰
    if (window.location.search.includes('debug')) {
        addLog('info', message);
    }
}

// 同步注入方式选择（注入页 <-> 配置页）
function syncInjectMethod(value) {
    document.getElementById('cfgInjectMethod').value = value;
    persistState();
}

function syncCfgInjectMethod(value) {
    document.getElementById('injectMethod').value = value;
    persistState();
}

// 同步批量延迟（注入页 <-> 配置页）
function syncBatchDelay(value) {
    document.getElementById('cfgBatchDelay').value = value;
    persistState();
}

function syncCfgBatchDelay(value) {
    document.getElementById('batchDelay').value = value;
    persistState();
}

// 打开日志文件夹
async function openLogFolder() {
    try {
        const result = await invoke('open_log_folder');
        if (result.success) {
            addLog('info', `已打开日志文件夹: ${result.path}`);
        } else {
            addLog('error', `打开日志文件夹失败: ${result.message}`);
        }
    } catch (err) {
        addLog('error', `打开日志文件夹失败: ${err}`);
    }
}

// 窗口控制
function minimizeWindow() {
    invoke('minimize');
}

function closeApp() {
    invoke('close');
}

// 初始化
document.addEventListener('DOMContentLoaded', () => {
    loadSettings();
    loadConfig();
    startProcessStatusRefresh();
    addLog('info', 'Tinject 初始化完成');
});
