cmake_minimum_required(VERSION 3.16)
project(smwapt_gui LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets)

qt_add_executable(smwapt-gui
  src/main.cpp
)

target_link_libraries(smwapt-gui PRIVATE Qt6::Widgets)
