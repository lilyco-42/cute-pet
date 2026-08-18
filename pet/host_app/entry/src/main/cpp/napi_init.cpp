// 丛雨桌宠 — 鸿蒙宿主壳 NAPI 桥接
//
// ArkTS XComponent surfaceId → OHNativeWindow → Rust ohos backend。
// Rust 侧导出符号(见 pet/src/main.rs pet_entry / vendor/miniquad-ply ohos.rs):
//   pet_entry()                      启动 macroquad → 渲染线程(忙等 surface)
//   ohos_surface_created(window)     喂 OHNativeWindow* 给渲染线程
//   ohos_surface_changed(w,h)        尺寸事件
//   ohos_surface_destroyed()         销毁事件
//   ohos_touch(x,y,id,down)          触摸事件
#include "napi/native_api.h"

#include <cstdint>

#include <native_window/external_window.h>

// 输入法(native IME): 软键盘弹起 + 上屏文本/删除回调 → Rust 输入框
#include <inputmethod/inputmethod_text_editor_proxy_capi.h>
#include <inputmethod/inputmethod_controller_capi.h>
#include <inputmethod/inputmethod_attach_options_capi.h>
#include <arkui/native_node_napi.h> // OH_ArkUI_GetContextFromNapiValue

// ---- Rust C ABI ----
extern "C" {
void pet_entry();
void ohos_surface_created(void* surface);
void ohos_surface_changed(int32_t width, int32_t height);
void ohos_surface_destroyed();
void ohos_touch(float x, float y, uint64_t touch_id, bool down);
void ohos_char(uint32_t ch);
void ohos_key(int32_t keycode, bool down);
void ohos_keyboard_height(int32_t px);
}

// hilog 日志(诊断用)
#include <hilog/log.h>
#undef LOG_DOMAIN
#undef LOG_TAG
#define LOG_DOMAIN 0xD003C00
#define LOG_TAG "CutePet"

// 当前 surface 尺寸(供键盘避让用)
static int32_t g_curSurfaceW = 0;
static int32_t g_curSurfaceH = 0;

// petStart(surfaceId: bigint, w: number, h: number): number
// 1) 启动 Rust 渲染线程(pet_entry 内部 spawn 后返回, 线程忙等首个 surface)
// 2) surfaceId → OHNativeWindow → ohos_surface_created → 渲染线程 EGL 初始化
static napi_value PetStart(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value args[3] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);

    int64_t surfaceId = 0;
    int32_t w = 0, h = 0;
    bool lossless = true;
    // ArkTS 侧传 BigInt(surfaceId) → 必须用 bigint getter
    napi_get_value_bigint_int64(env, args[0], &surfaceId, &lossless);
    napi_get_value_int32(env, args[1], &w);
    napi_get_value_int32(env, args[2], &h);
    OH_LOG_Print(LOG_APP, LOG_INFO, LOG_DOMAIN, LOG_TAG,
                 "petStart id=%lld w=%d h=%d", surfaceId, w, h);

    // 启动渲染线程
    pet_entry();

    // surfaceId → OHNativeWindow(uint64 surfaceId, introduced API 12)
    OHNativeWindow* window = nullptr;
    int32_t ret = OH_NativeWindow_CreateNativeWindowFromSurfaceId(
        static_cast<uint64_t>(surfaceId), &window);
    if (ret != 0 || window == nullptr) {
        napi_value result;
        napi_create_int32(env, ret != 0 ? ret : -2, &result);
        return result;
    }
    ohos_surface_created(window);
    ohos_surface_changed(w, h);
    g_curSurfaceW = w;
    g_curSurfaceH = h;

    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petStop(): 销毁 surface
static napi_value PetStop(napi_env env, napi_callback_info info) {
    ohos_surface_destroyed();
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petResize(w: number, h: number): 尺寸变化
static napi_value PetResize(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int32_t w = 0, h = 0;
    napi_get_value_int32(env, args[0], &w);
    napi_get_value_int32(env, args[1], &h);
    ohos_surface_changed(w, h);
    g_curSurfaceW = w;
    g_curSurfaceH = h;
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petChar(ch: number): 字符输入(键盘/软键盘 → 输入框)
static napi_value PetChar(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    uint32_t ch = 0;
    napi_get_value_uint32(env, args[0], &ch);
    ohos_char(ch);
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petKey(keycode: number, down: boolean): 特殊键(退格/回车/Tab)
static napi_value PetKey(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int32_t keycode = 0;
    bool down = false;
    napi_get_value_int32(env, args[0], &keycode);
    napi_get_value_bool(env, args[1], &down);
    ohos_key(keycode, down);
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petKeyboard(px: number): 键盘高度(px, 0=隐藏) → Rust 布局避让
static napi_value PetKeyboard(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int32_t px = 0;
    napi_get_value_int32(env, args[0], &px);
    ohos_keyboard_height(px);
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// ---- 输入法(native IME) ----
static InputMethod_TextEditorProxy* g_imeProxy = nullptr;
static InputMethod_InputMethodProxy* g_imeInputMethodProxy = nullptr;

// 上屏文本(UTF-16) → 逐码点 ohos_char(Rust 输入框)
static void OnImeInsertText(InputMethod_TextEditorProxy* proxy, const char16_t* text, size_t length) {
    for (size_t i = 0; i < length; i++) {
        uint32_t cp = static_cast<uint32_t>(text[i]);
        if (cp >= 0xD800 && cp <= 0xDBFF && i + 1 < length) {
            uint32_t low = static_cast<uint32_t>(text[i + 1]);
            if (low >= 0xDC00 && low <= 0xDFFF) {
                cp = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                i++;
            }
        }
        ohos_char(cp);
    }
}

// 退格删除 → ohos_key(Backspace)
static void OnImeDeleteBackward(InputMethod_TextEditorProxy* proxy, int32_t length) {
    for (int32_t i = 0; i < length; i++) {
        ohos_key(2055, true); // KEYCODE_DEL
    }
}

static void OnImeDeleteForward(InputMethod_TextEditorProxy* proxy, int32_t length) {
    // 前向删除暂不支持(Rust 输入框仅退格)
    (void)proxy;
    (void)length;
}

// 输入法请求输入框配置(attach 时必调, 未设置会导致 Attach 返回 NULL_POINTER)
static void OnImeGetTextConfig(InputMethod_TextEditorProxy* proxy, InputMethod_TextConfig* config) {
    OH_TextConfig_SetInputType(config, IME_TEXT_INPUT_TYPE_TEXT);
    OH_TextConfig_SetEnterKeyType(config, IME_ENTER_KEY_SEND);
}

// ---- 输入法要求实现/注册的全部回调(缺失任一 → Attach 返回 NULL_POINTER) ----
// 光标文本查询: Rust 输入框文本在 native 侧不可读, 返回空/0
static void OnImeGetLeftTextOfCursor(InputMethod_TextEditorProxy* proxy, int32_t number, char16_t text[], size_t* length) {
    (void)proxy; (void)number; (void)text;
    if (length != nullptr) *length = 0;
}
static void OnImeGetRightTextOfCursor(InputMethod_TextEditorProxy* proxy, int32_t number, char16_t text[], size_t* length) {
    (void)proxy; (void)number; (void)text;
    if (length != nullptr) *length = 0;
}
static int32_t OnImeGetTextIndexAtCursor(InputMethod_TextEditorProxy* proxy) {
    (void)proxy;
    return 0;
}
static void OnImeMoveCursor(InputMethod_TextEditorProxy* proxy, InputMethod_Direction direction) {
    (void)proxy; (void)direction;
}
static void OnImeHandleSetSelection(InputMethod_TextEditorProxy* proxy, int32_t start, int32_t end) {
    (void)proxy; (void)start; (void)end;
}
static void OnImeSendEnterKey(InputMethod_TextEditorProxy* proxy, InputMethod_EnterKeyType enterKeyType) {
    (void)proxy; (void)enterKeyType;
    ohos_key(2054, true); // Enter → 发送
}
// 键盘状态: 仅记录(供 ArkTS 侧检测), surface 缩放由 ArkTS 改 XComponent 高度驱动
// (手动 ohos_surface_changed 会与 EGL surface 尺寸不匹配, 破坏渲染)
static void OnImeSendKeyboardStatus(InputMethod_TextEditorProxy* proxy, InputMethod_KeyboardStatus status) {
    (void)proxy; (void)status;
}
static int32_t OnImeReceivePrivateCommand(InputMethod_TextEditorProxy* proxy, InputMethod_PrivateCommand* privateCommand[], size_t size) {
    (void)proxy; (void)privateCommand; (void)size;
    return 0;
}
// 预上屏(拼音中间态): 不转发到输入框 — 只等确认后的 InsertText(否则输入框
// 会同时显示拼音和中文)
static int32_t OnImeSetPreviewText(InputMethod_TextEditorProxy* proxy, const char16_t text[], size_t length, int32_t start, int32_t end) {
    (void)proxy; (void)text; (void)length; (void)start; (void)end;
    return 0;
}
static void OnImeFinishTextPreview(InputMethod_TextEditorProxy* proxy) {
    (void)proxy;
}
static void OnImeHandleExtendAction(InputMethod_TextEditorProxy* proxy, InputMethod_ExtendAction action) {
    (void)proxy; (void)action;
}

static void SetupImeCallbacks(InputMethod_TextEditorProxy* proxy) {
    OH_TextEditorProxy_SetInsertTextFunc(proxy, OnImeInsertText);
    OH_TextEditorProxy_SetDeleteBackwardFunc(proxy, OnImeDeleteBackward);
    OH_TextEditorProxy_SetDeleteForwardFunc(proxy, OnImeDeleteForward);
    OH_TextEditorProxy_SetGetTextConfigFunc(proxy, OnImeGetTextConfig);
    OH_TextEditorProxy_SetGetLeftTextOfCursorFunc(proxy, OnImeGetLeftTextOfCursor);
    OH_TextEditorProxy_SetGetRightTextOfCursorFunc(proxy, OnImeGetRightTextOfCursor);
    OH_TextEditorProxy_SetGetTextIndexAtCursorFunc(proxy, OnImeGetTextIndexAtCursor);
    OH_TextEditorProxy_SetMoveCursorFunc(proxy, OnImeMoveCursor);
    OH_TextEditorProxy_SetHandleSetSelectionFunc(proxy, OnImeHandleSetSelection);
    OH_TextEditorProxy_SetSendEnterKeyFunc(proxy, OnImeSendEnterKey);
    OH_TextEditorProxy_SetSendKeyboardStatusFunc(proxy, OnImeSendKeyboardStatus);
    OH_TextEditorProxy_SetReceivePrivateCommandFunc(proxy, OnImeReceivePrivateCommand);
    OH_TextEditorProxy_SetSetPreviewTextFunc(proxy, OnImeSetPreviewText);
    OH_TextEditorProxy_SetFinishTextPreviewFunc(proxy, OnImeFinishTextPreview);
    OH_TextEditorProxy_SetHandleExtendActionFunc(proxy, OnImeHandleExtendAction);
}

// petAttachIme(uiContext): 创建 proxy + AttachWithUIContext → 弹系统软键盘。
// uiContext 为 ArkTS 侧 this.getUIContext()(否则 Attach 返回 NULL_POINTER)
static napi_value PetAttachIme(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);

    if (g_imeProxy == nullptr) {
        g_imeProxy = OH_TextEditorProxy_Create();
        if (g_imeProxy == nullptr) {
            napi_value result;
            napi_create_int32(env, -1, &result);
            return result;
        }
        SetupImeCallbacks(g_imeProxy);
    }
    if (g_imeInputMethodProxy != nullptr) {
        OH_InputMethodController_Detach(g_imeInputMethodProxy);
        g_imeInputMethodProxy = nullptr;
    }

    // 从 ArkTS 传入的 UIContext 拿 ArkUI_ContextHandle
    ArkUI_ContextHandle uiContext = nullptr;
    if (argc < 1 || args[0] == nullptr ||
        OH_ArkUI_GetContextFromNapiValue(env, args[0], &uiContext) != 0) {
        napi_value result;
        napi_create_int32(env, -3, &result);
        return result;
    }

    InputMethod_AttachOptions* opts = OH_AttachOptions_Create(true); // showKeyboard
    if (opts == nullptr) {
        napi_value result;
        napi_create_int32(env, -2, &result);
        return result;
    }
    InputMethod_ErrorCode ret = OH_InputMethodController_AttachWithUIContext(
        uiContext, g_imeProxy, opts, &g_imeInputMethodProxy);
    OH_AttachOptions_Destroy(opts);
    napi_value result;
    napi_create_int32(env, static_cast<int32_t>(ret), &result);
    return result;
}

// petDetachIme(): 收起软键盘
static napi_value PetDetachIme(napi_env env, napi_callback_info info) {
    if (g_imeInputMethodProxy != nullptr) {
        OH_InputMethodController_Detach(g_imeInputMethodProxy);
        g_imeInputMethodProxy = nullptr;
    }
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

// petTouch(x: number, y: number, id: number, down: boolean)
static napi_value PetTouch(napi_env env, napi_callback_info info) {
    size_t argc = 4;
    napi_value args[4] = {nullptr};
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    double x = 0, y = 0;
    int64_t id = 0;
    bool down = false;
    napi_get_value_double(env, args[0], &x);
    napi_get_value_double(env, args[1], &y);
    napi_get_value_int64(env, args[2], &id);
    napi_get_value_bool(env, args[3], &down);
    OH_LOG_Print(LOG_APP, LOG_INFO, 0xD003C00, "CutePet",
                 "petTouch x=%.0f y=%.0f id=%lld down=%d", x, y, id, down ? 1 : 0);
    ohos_touch(static_cast<float>(x), static_cast<float>(y),
               static_cast<uint64_t>(id), down);
    napi_value result;
    napi_create_int32(env, 0, &result);
    return result;
}

EXTERN_C_START
static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        {"petStart", nullptr, PetStart, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petStop", nullptr, PetStop, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petResize", nullptr, PetResize, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petTouch", nullptr, PetTouch, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petChar", nullptr, PetChar, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petKey", nullptr, PetKey, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petAttachIme", nullptr, PetAttachIme, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petDetachIme", nullptr, PetDetachIme, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"petKeyboard", nullptr, PetKeyboard, nullptr, nullptr, nullptr, napi_default, nullptr},
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}
EXTERN_C_END

static napi_module cutePetHostModule = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "cute_pet_host",
    .nm_priv = ((void*)0),
    .reserved = {0},
};

extern "C" __attribute__((constructor)) void RegisterCutePetHostModule(void) {
    napi_module_register(&cutePetHostModule);
}
