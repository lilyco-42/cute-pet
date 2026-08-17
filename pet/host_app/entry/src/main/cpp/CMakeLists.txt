cmake_minimum_required(VERSION 3.5.0)
project(cute_pet_host_app)

set(NATIVERENDER_ROOT_PATH ${CMAKE_CURRENT_SOURCE_DIR})

# Rust staticlib(桌宠渲染核心): entry/src/main/libs/<abi>/libcute_pet_host.a
set(RUST_CORE_A ${CMAKE_CURRENT_SOURCE_DIR}/../libs/${OHOS_ARCH}/libcute_pet_host.a)
message(STATUS "Rust core staticlib: ${RUST_CORE_A}")

find_library(napi-lib ace_napi.z)
find_library(native_window-lib native_window)
find_library(ohaudio-lib ohaudio)
find_library(hilog-lib hilog_ndk.z)
find_library(ohinputmethod-lib ohinputmethod)
find_library(ace-ndk ace_ndk.z)

add_library(cute_pet_host SHARED napi_init.cpp)

target_include_directories(cute_pet_host PRIVATE
  ${NATIVERENDER_ROOT_PATH}
)

# 注意链接顺序: Rust staticlib 在 ohaudio/hilog 之前(--as-needed 下
# 库需在引用它的目标之后, 否则被丢弃导致 undefined symbol)
target_link_libraries(cute_pet_host PUBLIC
  ${napi-lib}
  ${native_window-lib}
  ${RUST_CORE_A}
  ${ohaudio-lib}
  ${hilog-lib}
  ${ohinputmethod-lib}
  ${ace-ndk}
  m
)
target_link_options(cute_pet_host PRIVATE "-Wl,--as-needed")
