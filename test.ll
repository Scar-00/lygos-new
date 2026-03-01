; ModuleID = 'C:\dev\learning\lygos\lygos-util/../tests/simple_program.ly'
source_filename = "C:\\dev\\learning\\lygos\\lygos-util/../tests/simple_program.ly"
target datalayout = "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-windows-msvc"

define i32 @add(i32 %0, i32 %1) {
  %3 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  %4 = alloca i32, align 4
  store i32 %1, ptr %4, align 4
  %5 = alloca i32, align 4
  %binop-lhs = load i32, ptr %3, align 4
  %binop-rhs = load i32, ptr %4, align 4
  %6 = add i32 %binop-lhs, %binop-rhs
  store i32 %6, ptr %5, align 4
  %func-ret = load i32, ptr %5, align 4
  ret i32 %func-ret
}
