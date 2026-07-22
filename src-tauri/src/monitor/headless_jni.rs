//! Android：为 tauri-plugin-background-service 的 HeadlessBridge 提供 JNI 桩。
//!
//! 该插件的 LifecycleService 在启动前台服务时会强制 `bridge.start()`，默认
//! `System.loadLibrary("app_core")`。以太测原生库名为 `stock_predict_lib`，
//! 由 MainActivity 设置 `HeadlessBridge.nativeLibName`，本模块导出所需 JNI 符号，
//! 使 FGS 启动成功；真正盯盘逻辑仍由进程内 `BackgroundService` 承担。

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

const OK_RUNNING: &str =
    r#"{"ok":true,"state":"running","message":null,"recoverable":false}"#;

fn ok_string(env: &mut JNIEnv) -> jstring {
    env.new_string(OK_RUNNING)
        .expect("jni string")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_backgroundservice_HeadlessBridge_startCore<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _reason: JString<'local>,
) -> jstring {
    ok_string(&mut env)
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_backgroundservice_HeadlessBridge_stopCore<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _reason: JString<'local>,
) -> jstring {
    ok_string(&mut env)
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_backgroundservice_HeadlessBridge_notifyNetworkChanged<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    ok_string(&mut env)
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_backgroundservice_HeadlessBridge_callAction<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _call_id: JString<'local>,
    _action: JString<'local>,
) -> jstring {
    ok_string(&mut env)
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_backgroundservice_HeadlessBridge_notificationAction<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _action: JString<'local>,
    _chat_id: JString<'local>,
    _message_id: JString<'local>,
    _reply_text: JString<'local>,
) -> jstring {
    ok_string(&mut env)
}
