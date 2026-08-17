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

// ---- Rust C ABI ----
extern "C" {
void pet_entry();
void ohos_surface_created(void* surface);
void ohos_surface_changed(int32_t width, int32_t height);
void ohos_surface_destroyed();
void ohos_touch(float x, float y, uint64_t touch_id, bool down);
}

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
