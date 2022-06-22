if("${CMAKE_SYSTEM_NAME}" STREQUAL "Windows")
    set(Rust_CARGO_TARGET "x86_64-pc-windows-gnu")
elseif("${CMAKE_SYSTEM_NAME}" STREQUAL "Linux")
    set(Rust_CARGO_TARGET "x86_64-unknown-linux-gnu")
elseif("${CMAKE_SYSTEM_NAME}" STREQUAL "Darwin")
    # TODO: set correct target; here only for M1 on Mac
    # set(Rust_CARGO_TARGET "x86_64-apple-darwin")
    set(Rust_CARGO_TARGET "aarch64-apple-darwin")
else()
    message(FATAL_ERROR "hardcoded platformchecks not supported outside windows-gnu and linux-gnu")
endif()

corrosion_import_crate(MANIFEST_PATH "chronik_cxxbridge/Cargo.toml")

set(CXXBRIDGE_BINARY_FOLDER ${CMAKE_BINARY_DIR}/cargo/build/${Rust_CARGO_TARGET}/cxxbridge) 

add_custom_command(
    DEPENDS chronik_cxxbridge-static
    OUTPUT
        ${CXXBRIDGE_BINARY_FOLDER}/chronik_cxxbridge/src/lib.rs.cc
)

add_library(chronik
    chronik_cpp/chronik.cpp
    chronik_cpp/chronik_bridge.cpp
    ${CXXBRIDGE_BINARY_FOLDER}/chronik_cxxbridge/src/lib.rs.cc
)

target_include_directories(chronik
PRIVATE
    ${CXXBRIDGE_BINARY_FOLDER}
    ..
    .
)
target_link_libraries(chronik
    chronik_cxxbridge
    util
    leveldb
)
