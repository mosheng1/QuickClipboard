use std::env;

pub const AUTO_START_ARG: &str = "--autostart";
pub const ADMIN_RELAUNCH_ARG: &str = "--admin-relaunch";
pub const UNINSTALL_CLEANUP_ARG: &str = "--uninstall-cleanup";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupLaunchContext {
    pub from_auto_start: bool,
    pub admin_relaunch: bool,
}

pub fn launch_context() -> StartupLaunchContext {
    let mut context = StartupLaunchContext::default();
    for argument in env::args().skip(1) {
        match argument.as_str() {
            AUTO_START_ARG => context.from_auto_start = true,
            ADMIN_RELAUNCH_ARG => context.admin_relaunch = true,
            _ => {}
        }
    }
    context
}

pub fn is_uninstall_cleanup_requested() -> bool {
    env::args()
        .skip(1)
        .any(|argument| argument == UNINSTALL_CLEANUP_ARG)
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{ADMIN_RELAUNCH_ARG, AUTO_START_ARG};
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;
    use std::env;
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use windows::core::{Interface, BSTR, HRESULT, PCWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, RPC_E_CHANGED_MODE, VARIANT_FALSE, VARIANT_TRUE,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::TaskScheduler::{
        IActionCollection, IExecAction, ILogonTrigger, IRegisteredTask, ITaskDefinition,
        ITaskFolder, ITaskService, TaskScheduler, TASK_ACTION_EXEC, TASK_CREATE_OR_UPDATE,
        TASK_INSTANCES_PARALLEL, TASK_LOGON_INTERACTIVE_TOKEN, TASK_RUNLEVEL_HIGHEST,
        TASK_TRIGGER_LOGON,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_BINARY};
    use winreg::{RegKey, RegValue};

    const APP_NAME: &str = "QuickClipboard";
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const STARTUP_APPROVED_KEY: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    const STARTUP_STATE_KEY: &str = r"Software\QuickClipboard\Startup";
    const ADMIN_TASK_NAME_VALUE: &str = "AdminTaskName";
    const LEGACY_ADMIN_TASK_NAME: &str = "QuickClipboardAdmin";
    const ADMIN_TASK_PREFIX: &str = "QuickClipboardAdmin-";

    struct ComApartment {
        should_uninitialize: bool,
    }

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            // SAFETY: 仅初始化当前线程的 COM apartment，并在成功初始化时配对释放。
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if result == RPC_E_CHANGED_MODE {
                return Ok(Self {
                    should_uninitialize: false,
                });
            }
            result
                .ok()
                .map(|_| Self {
                    should_uninitialize: true,
                })
                .map_err(|error| format!("初始化 Windows 任务计划程序失败: {error}"))
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.should_uninitialize {
                // SAFETY: 与当前线程上成功的 CoInitializeEx 调用配对。
                unsafe { CoUninitialize() };
            }
        }
    }

    struct TaskClient {
        service: ITaskService,
        root: ITaskFolder,
        user: BSTR,
        domain: BSTR,
        task_name: String,
        _apartment: ComApartment,
    }

    impl TaskClient {
        fn connect() -> Result<Self, String> {
            let apartment = ComApartment::initialize()?;
            let empty = VARIANT::default();

            // SAFETY: TaskScheduler 是系统注册的 COM 类，返回的接口由 windows crate 管理生命周期。
            let service: ITaskService =
                unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }
                    .map_err(|error| format!("连接 Windows 任务计划程序失败: {error}"))?;

            // SAFETY: 传入 VT_EMPTY 表示连接本机并使用当前交互用户。
            unsafe { service.Connect(&empty, &empty, &empty, &empty) }
                .map_err(|error| format!("连接本机任务计划程序失败: {error}"))?;

            // SAFETY: service 已成功连接，根目录和当前用户均由任务计划程序返回。
            let root = unsafe { service.GetFolder(&BSTR::from(r"\")) }
                .map_err(|error| format!("打开任务计划程序根目录失败: {error}"))?;
            let user = unsafe { service.ConnectedUser() }
                .map_err(|error| format!("获取当前任务用户失败: {error}"))?;
            let domain = unsafe { service.ConnectedDomain() }
                .map_err(|error| format!("获取当前任务用户域失败: {error}"))?;
            let task_name = task_name_for_user(&user.to_string());

            Ok(Self {
                service,
                root,
                user,
                domain,
                task_name,
                _apartment: apartment,
            })
        }

        fn get_task(&self, name: &str) -> Result<Option<IRegisteredTask>, String> {
            // SAFETY: 根目录接口有效，任务名由本模块生成或经过格式校验。
            match unsafe { self.root.GetTask(&BSTR::from(name)) } {
                Ok(task) => Ok(Some(task)),
                Err(error) if is_not_found_error(&error) => Ok(None),
                Err(error) => Err(format!("查询管理员启动任务失败: {error}")),
            }
        }

        fn delete_task_if_exists(&self, name: &str) -> Result<(), String> {
            // SAFETY: 根目录接口有效，删除范围限制在本应用管理的任务名内。
            match unsafe { self.root.DeleteTask(&BSTR::from(name), 0) } {
                Ok(()) => Ok(()),
                Err(error) if is_not_found_error(&error) => Ok(()),
                Err(error) => Err(format!("删除管理员启动任务失败: {error}")),
            }
        }
    }

    pub fn is_running_as_admin() -> bool {
        // SAFETY: 令牌句柄只在本函数内使用，并在所有成功打开的路径上关闭。
        unsafe {
            let mut token_handle = Default::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_err() {
                return false;
            }

            let mut elevation = TOKEN_ELEVATION::default();
            let mut return_length = 0;
            let result = GetTokenInformation(
                token_handle,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut return_length,
            );
            let _ = CloseHandle(token_handle);

            result.is_ok() && elevation.TokenIsElevated != 0
        }
    }

    pub fn configure_auto_start(enabled: bool, run_as_admin: bool) -> Result<(), String> {
        if run_as_admin {
            if !is_running_as_admin() {
                return Err("配置管理员自启动需要先以管理员权限运行".to_string());
            }
            ensure_admin_task(enabled)?;
            disable_registry_auto_start()?;
        } else if enabled {
            enable_registry_auto_start()?;
        } else {
            disable_registry_auto_start()?;
        }
        Ok(())
    }

    pub fn repair_startup_configuration(
        auto_start: bool,
        run_as_admin: bool,
    ) -> Result<(), String> {
        if run_as_admin {
            if !is_running_as_admin() {
                return Ok(());
            }
            ensure_admin_task(auto_start)?;
            disable_registry_auto_start()?;
        } else {
            if stored_admin_task_name()?.is_some() {
                delete_admin_task()?;
            }
            if auto_start {
                ensure_registry_auto_start()?;
            } else {
                disable_registry_auto_start()?;
            }
        }
        Ok(())
    }

    pub fn switch_to_standard_mode(auto_start: bool) -> Result<(), String> {
        delete_admin_task()?;
        if auto_start {
            enable_registry_auto_start()
        } else {
            disable_registry_auto_start()
        }
    }

    pub fn get_auto_start_status(run_as_admin: bool) -> Result<bool, String> {
        if run_as_admin {
            is_admin_task_ready(true)
        } else {
            registry_auto_start_matches()
        }
    }

    pub fn is_admin_task_ready(auto_start: bool) -> Result<bool, String> {
        let client = TaskClient::connect()?;
        admin_task_matches(&client, auto_start)
    }

    pub fn delete_admin_task() -> Result<(), String> {
        let client = TaskClient::connect()?;
        let mut task_names =
            HashSet::from([client.task_name.clone(), LEGACY_ADMIN_TASK_NAME.to_string()]);
        if let Some(stored_name) = stored_admin_task_name()? {
            if is_managed_task_name(&stored_name) {
                task_names.insert(stored_name);
            }
        }

        for task_name in task_names {
            client.delete_task_if_exists(&task_name)?;
        }
        clear_stored_admin_task_name()
    }

    pub fn cleanup_startup_entries() -> Result<(), String> {
        let task_result = delete_admin_task();
        let registry_result = disable_registry_auto_start();
        task_result.and(registry_result)
    }

    pub fn try_elevate_and_restart(auto_start: bool) -> Result<bool, String> {
        match try_run_admin_task(auto_start) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                eprintln!("复用管理员启动任务失败，将回退到 UAC 提权: {error}");
            }
        }
        Ok(launch_with_uac(auto_start))
    }

    fn ensure_registry_auto_start() -> Result<(), String> {
        if registry_auto_start_matches()? {
            return Ok(());
        }
        enable_registry_auto_start()
    }

    fn enable_registry_auto_start() -> Result<(), String> {
        let command = expected_registry_command(&current_exe()?)?;
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        let (run_key, _) = current_user
            .create_subkey(RUN_KEY)
            .map_err(|error| format!("打开开机自启动注册表失败: {error}"))?;
        run_key
            .set_value(APP_NAME, &command)
            .map_err(|error| format!("写入开机自启动注册表失败: {error}"))?;

        if let Ok(approved_key) =
            current_user.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_READ | KEY_SET_VALUE)
        {
            approved_key
                .set_raw_value(
                    APP_NAME,
                    &RegValue {
                        vtype: REG_BINARY,
                        bytes: vec![0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    },
                )
                .map_err(|error| format!("启用 Windows 启动项失败: {error}"))?;
        }
        Ok(())
    }

    fn disable_registry_auto_start() -> Result<(), String> {
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = current_user.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
            delete_registry_value_if_exists(&run_key, APP_NAME)
                .map_err(|error| format!("删除开机自启动注册表失败: {error}"))?;
        }
        if let Ok(approved_key) =
            current_user.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_SET_VALUE)
        {
            delete_registry_value_if_exists(&approved_key, APP_NAME)
                .map_err(|error| format!("清理 Windows 启动项状态失败: {error}"))?;
        }
        Ok(())
    }

    fn registry_auto_start_matches() -> Result<bool, String> {
        let expected = expected_registry_command(&current_exe()?)?;
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = match current_user.open_subkey_with_flags(RUN_KEY, KEY_READ) {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("读取开机自启动注册表失败: {error}")),
        };
        let actual = match run_key.get_value::<String, _>(APP_NAME) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(format!("读取开机自启动命令失败: {error}")),
        };
        if actual != expected {
            return Ok(false);
        }

        let approved_key = match current_user.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_READ)
        {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(error) => return Err(format!("读取 Windows 启动项状态失败: {error}")),
        };
        match approved_key.get_raw_value(APP_NAME) {
            Ok(value) => Ok(startup_approved_enabled(&value.bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(format!("读取 Windows 启动项状态失败: {error}")),
        }
    }

    fn expected_registry_command(exe_path: &Path) -> Result<String, String> {
        let path = exe_path.to_string_lossy();
        if path.contains('"') || path.contains('\0') {
            return Err("程序路径包含 Windows 启动项不支持的字符".to_string());
        }
        Ok(format!("\"{path}\" {AUTO_START_ARG}"))
    }

    fn startup_approved_enabled(bytes: &[u8]) -> bool {
        match bytes.first().copied() {
            Some(0x02) => true,
            Some(0x03) => false,
            _ if bytes.len() >= 8 => bytes.iter().rev().take(8).all(|byte| *byte == 0),
            _ => true,
        }
    }

    fn ensure_admin_task(auto_start: bool) -> Result<(), String> {
        if !is_running_as_admin() {
            return Err("创建管理员启动任务需要管理员权限".to_string());
        }

        let client = TaskClient::connect()?;
        let previous_task_name = stored_admin_task_name()?;
        if !admin_task_matches(&client, auto_start)? {
            register_admin_task(&client, auto_start)?;
        }
        if let Some(previous_task_name) = previous_task_name {
            if previous_task_name != client.task_name && is_managed_task_name(&previous_task_name) {
                client.delete_task_if_exists(&previous_task_name)?;
            }
        }
        store_admin_task_name(&client.task_name)?;
        client.delete_task_if_exists(LEGACY_ADMIN_TASK_NAME)
    }

    fn admin_task_matches(client: &TaskClient, auto_start: bool) -> Result<bool, String> {
        let Some(task) = client.get_task(&client.task_name)? else {
            return Ok(false);
        };

        // SAFETY: 所有 COM 接口均来自当前 TaskClient，输出参数由 BSTR/接口类型接管。
        unsafe {
            if task.Enabled().map_err(task_query_error)?.0 == 0 {
                return Ok(false);
            }
            let definition = task.Definition().map_err(task_query_error)?;
            if !principal_matches(&definition, &client.user, &client.domain)? {
                return Ok(false);
            }
            if !action_matches(&definition, auto_start)? {
                return Ok(false);
            }
            triggers_match(&definition, &client.user, &client.domain, auto_start)
        }
    }

    unsafe fn principal_matches(
        definition: &ITaskDefinition,
        expected_user: &BSTR,
        expected_domain: &BSTR,
    ) -> Result<bool, String> {
        let principal = unsafe { definition.Principal() }.map_err(task_query_error)?;
        let mut run_level = Default::default();
        unsafe { principal.RunLevel(&mut run_level) }.map_err(task_query_error)?;
        if run_level != TASK_RUNLEVEL_HIGHEST {
            return Ok(false);
        }

        let mut user = BSTR::new();
        unsafe { principal.UserId(&mut user) }.map_err(task_query_error)?;
        Ok(task_user_matches(
            &user.to_string(),
            &expected_user.to_string(),
            &expected_domain.to_string(),
        ))
    }

    unsafe fn action_matches(
        definition: &ITaskDefinition,
        auto_start: bool,
    ) -> Result<bool, String> {
        let actions = unsafe { definition.Actions() }.map_err(task_query_error)?;
        let mut count = 0;
        unsafe { actions.Count(&mut count) }.map_err(task_query_error)?;
        if count != 1 {
            return Ok(false);
        }

        let action = unsafe { actions.get_Item(1) }.map_err(task_query_error)?;
        let exec: IExecAction = action.cast().map_err(task_query_error)?;
        let mut task_path = BSTR::new();
        unsafe { exec.Path(&mut task_path) }.map_err(task_query_error)?;
        let task_path = PathBuf::from(OsString::from_wide(&task_path));
        if !paths_match(&task_path, &current_exe()?) {
            return Ok(false);
        }

        let mut arguments = BSTR::new();
        unsafe { exec.Arguments(&mut arguments) }.map_err(task_query_error)?;
        Ok(arguments == admin_task_arguments(auto_start))
    }

    unsafe fn triggers_match(
        definition: &ITaskDefinition,
        expected_user: &BSTR,
        expected_domain: &BSTR,
        auto_start: bool,
    ) -> Result<bool, String> {
        let triggers = unsafe { definition.Triggers() }.map_err(task_query_error)?;
        let mut count = 0;
        unsafe { triggers.Count(&mut count) }.map_err(task_query_error)?;
        if !auto_start {
            return Ok(count == 0);
        }
        if count != 1 {
            return Ok(false);
        }

        let trigger = unsafe { triggers.get_Item(1) }.map_err(task_query_error)?;
        let logon: ILogonTrigger = trigger.cast().map_err(task_query_error)?;
        let mut user = BSTR::new();
        unsafe { logon.UserId(&mut user) }.map_err(task_query_error)?;
        Ok(task_user_matches(
            &user.to_string(),
            &expected_user.to_string(),
            &expected_domain.to_string(),
        ))
    }

    fn register_admin_task(client: &TaskClient, auto_start: bool) -> Result<(), String> {
        let exe = current_exe()?;

        // SAFETY: 任务定义的所有字段均使用独立的 Path、Arguments 和当前交互用户设置。
        unsafe {
            let definition = client
                .service
                .NewTask(0)
                .map_err(|error| format!("创建管理员任务定义失败: {error}"))?;

            let info = definition
                .RegistrationInfo()
                .map_err(|error| format!("初始化管理员任务信息失败: {error}"))?;
            info.SetAuthor(&BSTR::from(APP_NAME))
                .map_err(|error| format!("设置管理员任务作者失败: {error}"))?;
            info.SetDescription(&BSTR::from(
                "用于 QuickClipboard 在管理员窗口中响应快捷键和模拟粘贴",
            ))
            .map_err(|error| format!("设置管理员任务说明失败: {error}"))?;

            let settings = definition
                .Settings()
                .map_err(|error| format!("初始化管理员任务设置失败: {error}"))?;
            settings
                .SetAllowDemandStart(VARIANT_TRUE)
                .map_err(|error| format!("设置管理员任务按需启动失败: {error}"))?;
            settings
                .SetMultipleInstances(TASK_INSTANCES_PARALLEL)
                .map_err(|error| format!("设置管理员任务多实例策略失败: {error}"))?;
            settings
                .SetDisallowStartIfOnBatteries(VARIANT_FALSE)
                .map_err(|error| format!("设置管理员任务电池策略失败: {error}"))?;
            settings
                .SetStopIfGoingOnBatteries(VARIANT_FALSE)
                .map_err(|error| format!("设置管理员任务电池策略失败: {error}"))?;
            settings
                .SetStartWhenAvailable(VARIANT_TRUE)
                .map_err(|error| format!("设置管理员任务补偿启动失败: {error}"))?;
            settings
                .SetExecutionTimeLimit(&BSTR::from("PT0S"))
                .map_err(|error| format!("设置管理员任务运行时限失败: {error}"))?;
            settings
                .SetPriority(4)
                .map_err(|error| format!("设置管理员任务优先级失败: {error}"))?;
            settings
                .SetEnabled(VARIANT_TRUE)
                .map_err(|error| format!("启用管理员任务失败: {error}"))?;

            let principal = definition
                .Principal()
                .map_err(|error| format!("初始化管理员任务用户失败: {error}"))?;
            principal
                .SetUserId(&client.user)
                .map_err(|error| format!("设置管理员任务用户失败: {error}"))?;
            principal
                .SetLogonType(TASK_LOGON_INTERACTIVE_TOKEN)
                .map_err(|error| format!("设置管理员任务登录类型失败: {error}"))?;
            principal
                .SetRunLevel(TASK_RUNLEVEL_HIGHEST)
                .map_err(|error| format!("设置管理员任务权限级别失败: {error}"))?;

            let triggers = definition
                .Triggers()
                .map_err(|error| format!("初始化管理员任务触发器失败: {error}"))?;
            if auto_start {
                let trigger = triggers
                    .Create(TASK_TRIGGER_LOGON)
                    .map_err(|error| format!("创建登录触发器失败: {error}"))?;
                let logon: ILogonTrigger = trigger
                    .cast()
                    .map_err(|error| format!("配置登录触发器失败: {error}"))?;
                logon
                    .SetUserId(&client.user)
                    .map_err(|error| format!("设置登录触发用户失败: {error}"))?;
            }

            configure_exec_action(
                &definition.Actions().map_err(task_query_error)?,
                &exe,
                auto_start,
            )?;

            let empty = VARIANT::default();
            client
                .root
                .RegisterTaskDefinition(
                    &BSTR::from(&client.task_name),
                    &definition,
                    TASK_CREATE_OR_UPDATE.0,
                    &empty,
                    &empty,
                    TASK_LOGON_INTERACTIVE_TOKEN,
                    &empty,
                )
                .map_err(|error| format!("注册管理员启动任务失败: {error}"))?;
        }
        Ok(())
    }

    unsafe fn configure_exec_action(
        actions: &IActionCollection,
        exe: &Path,
        auto_start: bool,
    ) -> Result<(), String> {
        let action = unsafe { actions.Create(TASK_ACTION_EXEC) }
            .map_err(|error| format!("创建管理员任务动作失败: {error}"))?;
        let exec: IExecAction = action
            .cast()
            .map_err(|error| format!("配置管理员任务动作失败: {error}"))?;
        unsafe { exec.SetPath(&bstr_from_os(exe.as_os_str())) }
            .map_err(|error| format!("设置管理员任务程序路径失败: {error}"))?;
        unsafe { exec.SetArguments(&BSTR::from(admin_task_arguments(auto_start))) }
            .map_err(|error| format!("设置管理员任务启动参数失败: {error}"))?;
        if let Some(parent) = exe.parent() {
            unsafe { exec.SetWorkingDirectory(&bstr_from_os(parent.as_os_str())) }
                .map_err(|error| format!("设置管理员任务工作目录失败: {error}"))?;
        }
        Ok(())
    }

    fn try_run_admin_task(auto_start: bool) -> Result<bool, String> {
        let client = TaskClient::connect()?;
        if !admin_task_matches(&client, auto_start)? {
            return Ok(false);
        }
        let Some(task) = client.get_task(&client.task_name)? else {
            return Ok(false);
        };
        let empty = VARIANT::default();

        // SAFETY: 任务已通过路径、参数、用户和最高权限校验。
        unsafe { task.Run(&empty) }.map_err(|error| format!("运行管理员启动任务失败: {error}"))?;
        // 任务提交成功后由任务计划程序负责启动。普通进程可能无权查询管理员任务的 PID，
        // 且桥接实例可能在通知已有实例后立即退出，不能据此回退到 UAC。
        Ok(true)
    }

    fn launch_with_uac(auto_start: bool) -> bool {
        let Ok(exe) = current_exe() else {
            return false;
        };
        let arguments = admin_task_arguments(auto_start);
        let operation = wide_null(OsStr::new("runas"));
        let file = wide_null(exe.as_os_str());
        let parameters = wide_null(OsStr::new(&arguments));
        let directory = exe.parent().map(|path| wide_null(path.as_os_str()));
        let directory_ptr = directory
            .as_ref()
            .map(|value| PCWSTR(value.as_ptr()))
            .unwrap_or(PCWSTR::null());

        // SAFETY: 所有 UTF-16 参数均以 NUL 结尾，并在调用期间保持有效。
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(parameters.as_ptr()),
                directory_ptr,
                SW_SHOWNORMAL,
            )
        };
        result.0 as usize > 32
    }

    fn admin_task_arguments(auto_start: bool) -> String {
        if auto_start {
            format!("{ADMIN_RELAUNCH_ARG} {AUTO_START_ARG}")
        } else {
            ADMIN_RELAUNCH_ARG.to_string()
        }
    }

    fn current_exe() -> Result<PathBuf, String> {
        let path = env::current_exe().map_err(|error| format!("获取程序路径失败: {error}"))?;
        if !path.is_file() {
            return Err("当前程序路径不存在，无法配置启动项".to_string());
        }
        Ok(path)
    }

    fn paths_match(left: &Path, right: &Path) -> bool {
        comparable_path(left) == comparable_path(right)
    }

    fn comparable_path(path: &Path) -> String {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let text = path.to_string_lossy().replace('/', r"\");
        text.strip_prefix(r"\\?\")
            .unwrap_or(&text)
            .trim_end_matches('\\')
            .to_lowercase()
    }

    fn task_name_for_user(user: &str) -> String {
        let digest = Sha256::digest(user.trim().to_lowercase().as_bytes());
        format!("{ADMIN_TASK_PREFIX}{}", hex::encode(&digest[..8]))
    }

    fn task_user_matches(actual: &str, expected_user: &str, expected_domain: &str) -> bool {
        let actual = actual.trim();
        let expected_user = expected_user.trim();
        if actual.eq_ignore_ascii_case(expected_user) {
            return true;
        }

        let expected_domain = expected_domain.trim();
        let (expected_qualified_domain, expected_account) = expected_user
            .rsplit_once('\\')
            .unwrap_or((expected_domain, expected_user));

        match actual.rsplit_once('\\') {
            Some((actual_domain, actual_user)) => {
                actual_user.eq_ignore_ascii_case(expected_account)
                    && actual_domain.eq_ignore_ascii_case(expected_qualified_domain)
            }
            None => actual.eq_ignore_ascii_case(expected_account),
        }
    }

    fn is_managed_task_name(task_name: &str) -> bool {
        let Some(suffix) = task_name.strip_prefix(ADMIN_TASK_PREFIX) else {
            return task_name == LEGACY_ADMIN_TASK_NAME;
        };
        suffix.len() == 16 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    fn store_admin_task_name(task_name: &str) -> Result<(), String> {
        if !is_managed_task_name(task_name) {
            return Err("管理员启动任务名称无效".to_string());
        }
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = current_user
            .create_subkey(STARTUP_STATE_KEY)
            .map_err(|error| format!("保存管理员任务信息失败: {error}"))?;
        key.set_value(ADMIN_TASK_NAME_VALUE, &task_name)
            .map_err(|error| format!("保存管理员任务名称失败: {error}"))
    }

    fn stored_admin_task_name() -> Result<Option<String>, String> {
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        let key = match current_user.open_subkey_with_flags(STARTUP_STATE_KEY, KEY_READ) {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("读取管理员任务信息失败: {error}")),
        };
        match key.get_value(ADMIN_TASK_NAME_VALUE) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("读取管理员任务名称失败: {error}")),
        }
    }

    fn clear_stored_admin_task_name() -> Result<(), String> {
        let current_user = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = current_user.open_subkey_with_flags(STARTUP_STATE_KEY, KEY_SET_VALUE) {
            delete_registry_value_if_exists(&key, ADMIN_TASK_NAME_VALUE)
                .map_err(|error| format!("清理管理员任务信息失败: {error}"))?;
        }
        Ok(())
    }

    fn delete_registry_value_if_exists(key: &RegKey, name: &str) -> io::Result<()> {
        match key.delete_value(name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn task_query_error(error: windows::core::Error) -> String {
        format!("校验管理员启动任务失败: {error}")
    }

    fn is_not_found_error(error: &windows::core::Error) -> bool {
        error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
    }

    fn bstr_from_os(value: &OsStr) -> BSTR {
        BSTR::from_wide(&value.encode_wide().collect::<Vec<_>>())
    }

    fn wide_null(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn registry_command_quotes_paths_and_preserves_unicode() {
            let path = Path::new(r"C:\程序 文件\QuickClipboard.exe");
            assert_eq!(
                expected_registry_command(path).unwrap(),
                r#""C:\程序 文件\QuickClipboard.exe" --autostart"#
            );
        }

        #[test]
        fn admin_task_arguments_match_trigger_mode() {
            assert_eq!(admin_task_arguments(false), "--admin-relaunch");
            assert_eq!(admin_task_arguments(true), "--admin-relaunch --autostart");
        }

        #[test]
        fn startup_approved_state_distinguishes_enabled_and_disabled() {
            assert!(startup_approved_enabled(&[
                0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]));
            assert!(!startup_approved_enabled(&[
                0x03, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8
            ]));
        }

        #[test]
        fn task_name_is_stable_and_restricted() {
            let task_name = task_name_for_user(r"DOMAIN\User");
            assert_eq!(task_name, task_name_for_user(r"domain\user"));
            assert!(is_managed_task_name(&task_name));
            assert!(!is_managed_task_name("QuickClipboardAdmin-../../OtherTask"));
        }

        #[test]
        fn task_user_accepts_windows_domain_normalization() {
            assert!(task_user_matches("TestUser", "TestUser", "EXAMPLE-PC"));
            assert!(task_user_matches(
                r"EXAMPLE-PC\testuser",
                "TestUser",
                "EXAMPLE-PC"
            ));
            assert!(task_user_matches(
                "TestUser",
                r"EXAMPLE-PC\TestUser",
                "EXAMPLE-PC"
            ));
            assert!(!task_user_matches(
                r"OTHER-PC\TestUser",
                "TestUser",
                "EXAMPLE-PC"
            ));
            assert!(!task_user_matches(
                r"EXAMPLE-PC\OtherUser",
                "TestUser",
                "EXAMPLE-PC"
            ));
        }
    }
}

#[cfg(target_os = "windows")]
pub use platform::{
    cleanup_startup_entries, configure_auto_start, get_auto_start_status, is_admin_task_ready,
    is_running_as_admin, repair_startup_configuration, switch_to_standard_mode,
    try_elevate_and_restart,
};

#[cfg(not(target_os = "windows"))]
pub fn is_running_as_admin() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn configure_auto_start(_enabled: bool, _run_as_admin: bool) -> Result<(), String> {
    Err("自启动配置目前仅支持 Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn repair_startup_configuration(_auto_start: bool, _run_as_admin: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn switch_to_standard_mode(_auto_start: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn get_auto_start_status(_run_as_admin: bool) -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn is_admin_task_ready(_auto_start: bool) -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn try_elevate_and_restart(_auto_start: bool) -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn cleanup_startup_entries() -> Result<(), String> {
    Ok(())
}
