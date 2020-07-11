cmake_minimum_required(VERSION 3.0)

project(remapper)

add_executable(remapper main.cpp)

install(TARGETS remapper RUNTIME DESTINATION bin)
