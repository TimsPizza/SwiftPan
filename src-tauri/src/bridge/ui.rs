//! Platform UI-information Tauri commands.
//!
//! This module owns native status-bar measurement on Android and iOS. It must
//! not own SAF file operations, general bridge dispatch, transfers, remote
//! objects, credentials, or other UI behavior.

use crate::types::SpResult;

#[tauri::command]
pub async fn ui_status_bar_height() -> SpResult<i32> {
    #[cfg(target_os = "android")]
    {
        use jni::objects::{JClass, JObject, JValue};
        use jni::JavaVM;

        unsafe {
            let context = ndk_context::android_context();
            let vm_pointer = context.vm();
            let context_object = context.context();
            if vm_pointer.is_null() || context_object.is_null() {
                return Ok(0);
            }

            let vm = JavaVM::from_raw(vm_pointer as *mut _)
                .map_err(|_| ())
                .unwrap_or_else(|_| unreachable!());
            let mut environment = match vm.attach_current_thread() {
                Ok(environment) => environment,
                Err(_) => return Ok(0),
            };

            let global_context = JObject::from_raw(context_object as jni::sys::jobject);
            let local_context = match environment.new_local_ref(&global_context) {
                Ok(object) => JObject::from(object),
                Err(_) => return Ok(0),
            };

            let version_class: JClass = match environment.find_class("android/os/Build$VERSION") {
                Ok(class) => class,
                Err(_) => return Ok(0),
            };
            let sdk_version = environment
                .get_static_field(version_class, "SDK_INT", "I")
                .ok()
                .and_then(|value| value.i().ok())
                .unwrap_or(0);

            let is_activity = environment
                .is_instance_of(&local_context, "android/app/Activity")
                .unwrap_or(false);
            if !is_activity {
                return Ok(status_bar_dimen_fallback(&mut environment, &local_context));
            }
            let activity = local_context;

            let window = environment
                .call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])
                .ok()
                .and_then(|value| value.l().ok());
            let Some(window) = window else {
                return Ok(status_bar_dimen_fallback(&mut environment, &activity));
            };

            let decor = environment
                .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
                .ok()
                .and_then(|value| value.l().ok());
            let Some(decor) = decor else {
                return Ok(status_bar_dimen_fallback(&mut environment, &activity));
            };

            let insets = environment
                .call_method(
                    &decor,
                    "getRootWindowInsets",
                    "()Landroid/view/WindowInsets;",
                    &[],
                )
                .ok()
                .and_then(|value| value.l().ok());
            let Some(insets) = insets else {
                return Ok(status_bar_dimen_fallback(&mut environment, &activity));
            };

            if sdk_version >= 30 {
                let type_class: JClass =
                    match environment.find_class("android/view/WindowInsets$Type") {
                        Ok(class) => class,
                        Err(_) => {
                            return Ok(status_bar_dimen_fallback(&mut environment, &activity));
                        }
                    };
                let status_bars = environment
                    .call_static_method(type_class, "statusBars", "()I", &[])
                    .ok()
                    .and_then(|value| value.i().ok())
                    .unwrap_or(0);
                let type_class: JClass =
                    match environment.find_class("android/view/WindowInsets$Type") {
                        Ok(class) => class,
                        Err(_) => {
                            return Ok(status_bar_dimen_fallback(&mut environment, &activity));
                        }
                    };
                let display_cutout = environment
                    .call_static_method(type_class, "displayCutout", "()I", &[])
                    .ok()
                    .and_then(|value| value.i().ok())
                    .unwrap_or(0);
                let mask = status_bars | display_cutout;

                let insets_object = environment
                    .call_method(
                        &insets,
                        "getInsets",
                        "(I)Landroid/graphics/Insets;",
                        &[JValue::from(mask)],
                    )
                    .ok()
                    .and_then(|value| value.l().ok());
                if let Some(insets_object) = insets_object {
                    let top = environment
                        .get_field(&insets_object, "top", "I")
                        .ok()
                        .and_then(|value| value.i().ok())
                        .unwrap_or(0);
                    return Ok(top.max(0));
                }
                return Ok(status_bar_dimen_fallback(&mut environment, &activity));
            }

            if sdk_version >= 23 {
                let top = environment
                    .call_method(&insets, "getSystemWindowInsetTop", "()I", &[])
                    .ok()
                    .and_then(|value| value.i().ok())
                    .unwrap_or(0);
                return Ok(if top > 0 {
                    top
                } else {
                    status_bar_dimen_fallback(&mut environment, &activity)
                });
            }

            return Ok(status_bar_dimen_fallback(&mut environment, &activity));
        }

        fn status_bar_dimen_fallback(environment: &mut jni::JNIEnv, context: &JObject) -> i32 {
            let resources = environment
                .call_method(
                    context,
                    "getResources",
                    "()Landroid/content/res/Resources;",
                    &[],
                )
                .ok()
                .and_then(|value| value.l().ok());
            let Some(resources) = resources else {
                return 0;
            };

            let name = environment.new_string("status_bar_height").ok();
            let dimension = environment.new_string("dimen").ok();
            let package = environment.new_string("android").ok();
            let (Some(name), Some(dimension), Some(package)) = (name, dimension, package) else {
                return 0;
            };

            let identifier = environment
                .call_method(
                    &resources,
                    "getIdentifier",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
                    &[
                        JValue::Object(&JObject::from(name)),
                        JValue::Object(&JObject::from(dimension)),
                        JValue::Object(&JObject::from(package)),
                    ],
                )
                .ok()
                .and_then(|value| value.i().ok())
                .unwrap_or(0);
            if identifier <= 0 {
                return 0;
            }
            environment
                .call_method(
                    &resources,
                    "getDimensionPixelSize",
                    "(I)I",
                    &[JValue::from(identifier)],
                )
                .ok()
                .and_then(|value| value.i().ok())
                .unwrap_or(0)
                .max(0)
        }
    }

    #[cfg(target_os = "ios")]
    {
        use objc::runtime::{Object, BOOL};
        use objc::{class, msg_send, sel, sel_impl};

        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGSize {
            width: f64,
            height: f64,
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGRect {
            origin: (f64, f64),
            size: CGSize,
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        struct UIEdgeInsets {
            top: f64,
            left: f64,
            bottom: f64,
            right: f64,
        }

        unsafe {
            let screen: *mut Object = msg_send![class!(UIScreen), mainScreen];
            let scale: f64 = if !screen.is_null() {
                let value: f64 = msg_send![screen, scale];
                if value > 0.0 {
                    value
                } else {
                    1.0
                }
            } else {
                1.0
            };

            let application: *mut Object = msg_send![class!(UIApplication), sharedApplication];
            let windows: *mut Object = msg_send![application, windows];
            let has_windows: BOOL = msg_send![windows, count];
            if has_windows as i32 > 0 {
                let window: *mut Object = msg_send![windows, firstObject];
                if !window.is_null() {
                    let insets: UIEdgeInsets = msg_send![window, safeAreaInsets];
                    let pixels = (insets.top * scale).round() as i32;
                    if pixels >= 0 {
                        return Ok(pixels);
                    }
                }
            }

            if has_windows as i32 > 0 {
                let window: *mut Object = msg_send![windows, firstObject];
                if !window.is_null() {
                    let scene: *mut Object = msg_send![window, windowScene];
                    if !scene.is_null() {
                        let manager: *mut Object = msg_send![scene, statusBarManager];
                        if !manager.is_null() {
                            let frame: CGRect = msg_send![manager, statusBarFrame];
                            let pixels = (frame.size.height * scale).round() as i32;
                            if pixels >= 0 {
                                return Ok(pixels);
                            }
                        }
                    }
                }
            }

            let frame: CGRect = msg_send![application, statusBarFrame];
            let pixels = (frame.size.height * scale).round() as i32;
            return Ok(pixels.max(0));
        }
    }

    #[allow(unreachable_code)]
    Ok(0)
}
