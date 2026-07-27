//! AndroidKeyStore adapter for wrapping the SwiftPan device key.
//!
//! This module owns JNI interaction with AES/GCM keys under the stable Android
//! alias. It must not serialize wrapped-key files, read credential packages,
//! resolve vault paths, or mutate runtime credentials.

use super::model::ANDROID_KEY_ALIAS;
use crate::types::{err_invalid, ErrorKind, SpError, SpResult};

pub(super) fn android_keystore_encrypt(plaintext: &[u8]) -> SpResult<(Vec<u8>, Vec<u8>)> {
    android_with_env(|environment| {
        let secret_key = android_keystore_secret_key(environment)?;
        let cipher = android_cipher(environment)?;
        environment
            .call_method(
                &cipher,
                "init",
                "(ILjava/security/Key;)V",
                &[
                    jni::objects::JValue::Int(1),
                    jni::objects::JValue::Object(secret_key.as_obj()),
                ],
            )
            .map_err(android_jni_err)?;
        let input = environment
            .byte_array_from_slice(plaintext)
            .map_err(android_jni_err)?;
        let ciphertext = environment
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                    input,
                ))],
            )
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        let iv = environment
            .call_method(&cipher, "getIV", "()[B", &[])
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        Ok((
            environment
                .convert_byte_array(jni::objects::JByteArray::from(iv))
                .map_err(android_jni_err)?,
            environment
                .convert_byte_array(jni::objects::JByteArray::from(ciphertext))
                .map_err(android_jni_err)?,
        ))
    })
}

pub(super) fn android_keystore_decrypt(iv: &[u8], ciphertext: &[u8]) -> SpResult<Vec<u8>> {
    android_with_env(|environment| {
        let secret_key = android_keystore_secret_key(environment)?;
        let cipher = android_cipher(environment)?;
        let iv_array = environment
            .byte_array_from_slice(iv)
            .map_err(android_jni_err)?;
        let spec = environment
            .new_object(
                "javax/crypto/spec/GCMParameterSpec",
                "(I[B)V",
                &[
                    jni::objects::JValue::Int(128),
                    jni::objects::JValue::Object(&jni::objects::JObject::from(iv_array)),
                ],
            )
            .map_err(android_jni_err)?;
        environment
            .call_method(
                &cipher,
                "init",
                "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
                &[
                    jni::objects::JValue::Int(2),
                    jni::objects::JValue::Object(secret_key.as_obj()),
                    jni::objects::JValue::Object(&spec),
                ],
            )
            .map_err(android_jni_err)?;
        let input = environment
            .byte_array_from_slice(ciphertext)
            .map_err(android_jni_err)?;
        let plaintext = environment
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                    input,
                ))],
            )
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        environment
            .convert_byte_array(jni::objects::JByteArray::from(plaintext))
            .map_err(android_jni_err)
    })
}

fn android_with_env<T>(operation: impl FnOnce(&mut jni::JNIEnv) -> SpResult<T>) -> SpResult<T> {
    use jni::JavaVM;

    unsafe {
        let context = ndk_context::android_context();
        let vm_pointer = context.vm();
        if vm_pointer.is_null() {
            return Err(err_invalid("android vm unavailable"));
        }
        let vm = JavaVM::from_raw(vm_pointer as *mut _).map_err(android_jni_err)?;
        let mut environment = vm.attach_current_thread().map_err(android_jni_err)?;
        operation(&mut environment)
    }
}

fn android_cipher(environment: &mut jni::JNIEnv) -> SpResult<jni::objects::GlobalRef> {
    let transformation = environment
        .new_string("AES/GCM/NoPadding")
        .map_err(android_jni_err)?;
    let cipher = environment
        .call_static_method(
            "javax/crypto/Cipher",
            "getInstance",
            "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                transformation,
            ))],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    environment.new_global_ref(cipher).map_err(android_jni_err)
}

fn android_keystore_secret_key(environment: &mut jni::JNIEnv) -> SpResult<jni::objects::GlobalRef> {
    let alias = environment
        .new_string(ANDROID_KEY_ALIAS)
        .map_err(android_jni_err)?;
    let provider = environment
        .new_string("AndroidKeyStore")
        .map_err(android_jni_err)?;
    let keystore = environment
        .call_static_method(
            "java/security/KeyStore",
            "getInstance",
            "(Ljava/lang/String;)Ljava/security/KeyStore;",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                provider,
            ))],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    environment
        .call_method(
            &keystore,
            "load",
            "(Ljava/io/InputStream;[C)V",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
            ],
        )
        .map_err(android_jni_err)?;
    let has_alias = environment
        .call_method(
            &keystore,
            "containsAlias",
            "(Ljava/lang/String;)Z",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                alias,
            ))],
        )
        .map_err(android_jni_err)?
        .z()
        .map_err(android_jni_err)?;
    if !has_alias {
        android_generate_keystore_key(environment)?;
    }
    let alias = environment
        .new_string(ANDROID_KEY_ALIAS)
        .map_err(android_jni_err)?;
    let entry = environment
        .call_method(
            &keystore,
            "getEntry",
            "(Ljava/lang/String;Ljava/security/KeyStore$ProtectionParameter;)Ljava/security/KeyStore$Entry;",
            &[
                jni::objects::JValue::Object(
                    &jni::objects::JObject::from(alias),
                ),
                jni::objects::JValue::Object(
                    &jni::objects::JObject::null(),
                ),
            ],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    let secret_key = environment
        .call_method(&entry, "getSecretKey", "()Ljavax/crypto/SecretKey;", &[])
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    environment
        .new_global_ref(secret_key)
        .map_err(android_jni_err)
}

fn android_generate_keystore_key(environment: &mut jni::JNIEnv) -> SpResult<()> {
    let algorithm = environment.new_string("AES").map_err(android_jni_err)?;
    let provider = environment
        .new_string("AndroidKeyStore")
        .map_err(android_jni_err)?;
    let generator = environment
        .call_static_method(
            "javax/crypto/KeyGenerator",
            "getInstance",
            "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::from(algorithm)),
                jni::objects::JValue::Object(&jni::objects::JObject::from(provider)),
            ],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    let encrypt = environment
        .get_static_field(
            "android/security/keystore/KeyProperties",
            "PURPOSE_ENCRYPT",
            "I",
        )
        .map_err(android_jni_err)?
        .i()
        .map_err(android_jni_err)?;
    let decrypt = environment
        .get_static_field(
            "android/security/keystore/KeyProperties",
            "PURPOSE_DECRYPT",
            "I",
        )
        .map_err(android_jni_err)?
        .i()
        .map_err(android_jni_err)?;
    let alias = environment
        .new_string(ANDROID_KEY_ALIAS)
        .map_err(android_jni_err)?;
    let builder = environment
        .new_object(
            "android/security/keystore/KeyGenParameterSpec$Builder",
            "(Ljava/lang/String;I)V",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::from(alias)),
                jni::objects::JValue::Int(encrypt | decrypt),
            ],
        )
        .map_err(android_jni_err)?;
    let block_modes = environment
        .new_object_array(1, "java/lang/String", jni::objects::JObject::null())
        .map_err(android_jni_err)?;
    let gcm = environment.new_string("GCM").map_err(android_jni_err)?;
    environment
        .set_object_array_element(&block_modes, 0, gcm)
        .map_err(android_jni_err)?;
    environment
        .call_method(
            &builder,
            "setBlockModes",
            "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                block_modes,
            ))],
        )
        .map_err(android_jni_err)?;
    let paddings = environment
        .new_object_array(1, "java/lang/String", jni::objects::JObject::null())
        .map_err(android_jni_err)?;
    let no_padding = environment
        .new_string("NoPadding")
        .map_err(android_jni_err)?;
    environment
        .set_object_array_element(&paddings, 0, no_padding)
        .map_err(android_jni_err)?;
    environment
        .call_method(
            &builder,
            "setEncryptionPaddings",
            "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                paddings,
            ))],
        )
        .map_err(android_jni_err)?;
    environment
        .call_method(
            &builder,
            "setKeySize",
            "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[jni::objects::JValue::Int(256)],
        )
        .map_err(android_jni_err)?;
    let spec = environment
        .call_method(
            &builder,
            "build",
            "()Landroid/security/keystore/KeyGenParameterSpec;",
            &[],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    environment
        .call_method(
            &generator,
            "init",
            "(Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[jni::objects::JValue::Object(&spec)],
        )
        .map_err(android_jni_err)?;
    environment
        .call_method(&generator, "generateKey", "()Ljavax/crypto/SecretKey;", &[])
        .map_err(android_jni_err)?;
    Ok(())
}

fn android_jni_err(error: impl std::fmt::Display) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("android keystore: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}
