cmake_minimum_required(VERSION 3.27)
project(pms C)

set(CMAKE_C_STANDARD 11)

add_executable(pms main.c)

install(TARGETS pms)
