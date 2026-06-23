// 系统电源状态检测
// 用于运行时判断是否跳过管理员提权自启动

// 检查系统是否正在使用电池供电（离电状态）
#[cfg(windows)]
pub fn is_on_battery() -> bool {
    #[repr(C)]
    struct SystemPowerStatus {
        ac_line_status: u8,
        battery_flag: u8,
        battery_life_percent: u8,
        reserved: u8,
        battery_life_time: u32,
        battery_full_life_time: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
    }

    let mut status = SystemPowerStatus {
        ac_line_status: 0,
        battery_flag: 0,
        battery_life_percent: 0,
        reserved: 0,
        battery_life_time: 0,
        battery_full_life_time: 0,
    };

    // ACLineStatus: 0 = 离电(电池), 1 = 接通电源, 255 = 未知
    if unsafe { GetSystemPowerStatus(&mut status) } != 0 {
        status.ac_line_status == 0
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn is_on_battery() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_on_battery_returns_bool() {
        // 仅验证函数可调用且不 panic，实际返回值取决于系统电源状态
        let _ = is_on_battery();
    }
}
