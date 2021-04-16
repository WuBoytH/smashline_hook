use smash::lua2cpp::*;
use smash::lib::{LuaConst, L2CValue};
use smash::phx::Hash40;

use skyline::nro::NroInfo;

use crate::LuaConstant;
use crate::c_str;

use std::collections::HashMap;
use parking_lot::Mutex;

type FighterFrame = extern "C" fn(&mut L2CFighterCommon) -> L2CValue;
type WeaponFrame = extern "C" fn(&mut L2CFighterBase) -> L2CValue;
type FighterReset = fn(&mut L2CFighterCommon);
type AgentReset = fn(&mut L2CFighterBase);

struct FighterFrameInfo {
    pub agent: LuaConstant,
    pub original: Option<&'static mut *const extern "C" fn()>,
    pub frame: FighterFrame
}

unsafe impl Sync for FighterFrameInfo {}
unsafe impl Send for FighterFrameInfo {}

struct WeaponFrameInfo {
    pub agent: LuaConstant,
    pub original: Option<&'static mut *const extern "C" fn()>,
    pub frame: WeaponFrame
}

unsafe impl Sync for WeaponFrameInfo {}
unsafe impl Send for WeaponFrameInfo {}

lazy_static! {
    static ref FIGHTER_FRAMES: Mutex<Vec<FighterFrameInfo>> = Mutex::new(Vec::new());
    static ref WEAPON_FRAMES: Mutex<Vec<WeaponFrameInfo>> = Mutex::new(Vec::new());

    static ref FIGHTER_RESETS: Mutex<Vec<FighterReset>> = Mutex::new(Vec::new());
    static ref AGENT_RESETS: Mutex<Vec<AgentReset>> = Mutex::new(Vec::new());
}

// These symbols must be used since they are passed va_lists
// call_check_attack, for example, does not take a va_list
extern "C" {
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon32bind_hash_call_call_check_damageEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_check_damage();
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon32bind_hash_call_call_check_attackEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_check_attack();
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon32bind_hash_call_call_on_change_lrEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_on_change_lr();
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon30bind_hash_call_call_leave_stopEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_leave_stop();
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon40bind_hash_call_call_notify_event_gimmickEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_notify_event_gimmick();
    #[link_name = "\u{1}_ZN7lua2cpp16L2CFighterCommon30bind_hash_call_call_calc_paramEPN3lib8L2CAgentERNS1_7utility8VariadicEPKcSt9__va_list"]
    fn call_calc_param();
}

unsafe extern "C" fn sys_line_system_fighter_init_replace(fighter: &mut L2CFighterCommon) -> L2CValue {
    use std::mem::transmute;
    fighter.sv_set_function_hash(transmute(call_check_damage as *const ()), Hash40::new("call_check_damage"));
    fighter.sv_set_function_hash(transmute(call_check_attack as *const ()), Hash40::new("call_check_attack"));
    fighter.sv_set_function_hash(transmute(call_on_change_lr as *const ()), Hash40::new("call_on_change_lr"));
    fighter.sv_set_function_hash(transmute(call_leave_stop as *const ()), Hash40::new("call_leave_stop"));
    fighter.sv_set_function_hash(transmute(call_notify_event_gimmick as *const ()), Hash40::new("call_notify_event_gimmick"));
    fighter.sv_set_function_hash(transmute(call_calc_param as *const ()), Hash40::new("call_calc_param"));

    // already been resolved by this point, so we have to do it this way unfortunately :(
    let mut sys_line_system_control = 0usize;
    skyline::nn::ro::LookupSymbol(&mut sys_line_system_control, c_str!("_ZN7lua2cpp16L2CFighterCommon31sys_line_system_control_fighterEv"));

    let mut system_control = L2CValue::Ptr(transmute(sys_line_system_control as *const ()));
    let mut fighter_frames = FIGHTER_FRAMES.lock();
    for frame_info in fighter_frames.iter_mut() {
        if frame_info.agent.get() == smash::app::utility::get_kind(&mut *fighter.module_accessor) {
            if let Some(original) = frame_info.original.as_mut() {
                let og = system_control.get_ptr() as *const extern "C" fn();
                **original = og;
            }
            system_control = L2CValue::Ptr(transmute(frame_info.frame as *const ()));
        }
    }
    drop(fighter_frames);
    fighter.shift(system_control.clone());
    let func: extern "C" fn(&mut L2CFighterCommon) -> L2CValue = transmute(system_control.get_ptr());
    func(fighter)
}

unsafe extern "C" fn sys_line_system_init_replace(agent: &mut L2CFighterBase) -> L2CValue {
    use std::mem::transmute;

    let mut sys_line_system_control = 0usize;
    skyline::nn::ro::LookupSymbol(&mut sys_line_system_control, c_str!("_ZN7lua2cpp14L2CFighterBase23sys_line_system_controlEv"));
    let mut system_control = L2CValue::Ptr(transmute(sys_line_system_control as *const ()));
    let mut weapon_frames = WEAPON_FRAMES.lock();
    for frame_info in weapon_frames.iter_mut() {
        if frame_info.agent.get() == smash::app::utility::get_kind(&mut *agent.module_accessor) {
            if let Some(original) = frame_info.original.as_mut() {
                let og = system_control.get_ptr() as *const extern "C" fn();
                **original = og;
            }
            system_control = L2CValue::Ptr(transmute(frame_info.frame as *const ()));
        }
    }
    drop(weapon_frames);
    agent.shift(system_control.clone());
    let func: extern "C" fn(&mut L2CFighterBase) -> L2CValue = transmute(system_control.get_ptr());
    func(agent)
}

#[skyline::hook(replace = L2CFighterCommon_RESET)]
fn fighter_reset(fighter: &mut L2CFighterCommon) {
    for callback in FIGHTER_RESETS.lock().iter() {
        callback(fighter)
    }
}

#[skyline::hook(replace = L2CFighterBase_RESET)]
fn agent_reset(agent: &mut L2CFighterBase) {
    for callback in AGENT_RESETS.lock().iter() {
        callback(agent)
    }
}

#[no_mangle]
pub extern "Rust" fn replace_fighter_frame(agent: LuaConstant, original: Option<&'static mut *const extern "C" fn()>, replacement: FighterFrame) {
    let info = FighterFrameInfo {
        agent,
        original,
        frame: replacement
    };
    let mut fighter_frames = FIGHTER_FRAMES.lock();
    fighter_frames.push(info);
}

#[no_mangle]
pub extern "Rust" fn replace_weapon_frame(agent: LuaConstant, original: Option<&'static mut *const extern "C" fn()>, replacement: WeaponFrame) {
    let info = WeaponFrameInfo {
        agent,
        original,
        frame: replacement
    };
    let mut weapon_frames = WEAPON_FRAMES.lock();
    weapon_frames.push(info);
}

#[no_mangle]
pub extern "Rust" fn add_fighter_reset_callback(callback: FighterReset) {
    FIGHTER_RESETS.lock().push(callback);
}

#[no_mangle]
pub extern "Rust" fn add_agent_reset_callback(callback: AgentReset) {
    AGENT_RESETS.lock().push(callback);
}

fn install() {
    crate::hooks::replace_symbol("common", "_ZN7lua2cpp16L2CFighterCommon20sys_line_system_initEv", sys_line_system_fighter_init_replace as *const extern "C" fn(), None);
    crate::hooks::replace_symbol("common", "_ZN7lua2cpp14L2CFighterBase20sys_line_system_initEv", sys_line_system_init_replace as *const extern "C" fn(), None);
    skyline::install_hooks!(
        fighter_reset,
        agent_reset
    );
}

pub fn nro_load(info: &NroInfo) {
    if info.name == "common" {
        install();
    }
}

pub fn nro_unload(_: &NroInfo) {

}