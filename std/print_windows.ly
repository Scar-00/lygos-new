// Adapt the sample's descriptor-based write API to the Windows CRT.
struct Stream {}
fn __acrt_iob_func(index:u32)->*mut Stream;
fn fwrite(data:*const i8,size:u64,count:u64,stream:*mut Stream)->u64;
fn write(file:u32,data:*const i8,length:u64)->i64 {
    return (:i64)fwrite(data,(:u64)1,length,__acrt_iob_func(file));
}
