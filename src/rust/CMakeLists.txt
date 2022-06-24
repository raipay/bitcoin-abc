find_package(Corrosion 0.2.0 REQUIRED)

add_custom_target(check-rust-test-suite
COMMAND
    Rust::Cargo
    test
WORKING_DIRECTORY
    ${CMAKE_CURRENT_SOURCE_DIR}
)
