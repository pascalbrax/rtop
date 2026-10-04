/* Test-only NVML ABI fixture. Never installed or distributed as a driver. */
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
static int mode(const char *s) { const char *m=getenv("RTOP_FAKE_NVML"); return m && !strcmp(m,s); }
int nvmlInit_v2(void) { if(mode("slow")) sleep(10); return mode("init-error")?9:0; }
int nvmlShutdown(void) { return 0; }
int nvmlDeviceGetCount_v2(unsigned *count) { *count=mode("zero")?0:1; return mode("count-error")?6:0; }
int nvmlDeviceGetHandleByIndex_v2(unsigned index, void **device) { (void)index; *device=mode("null")?NULL:(void*)(uintptr_t)1; return mode("permission")?4:0; }
#ifndef REQUIRED_ONLY
const char *nvmlErrorString(int code) { switch(code) { case 3:return "Not Supported"; case 4:return "Insufficient Permissions"; case 6:return "Device Not Found"; case 15:return "GPU Lost"; default:return "Unknown Error"; } }
int nvmlDeviceGetName(void *dev, char *s, unsigned n) { (void)dev; strncpy(s,"Fixture NVIDIA",n); return 0; }
int nvmlDeviceGetUUID(void *dev, char *s, unsigned n) { (void)dev; strncpy(s,"GPU-fixture-uuid",n); return 0; }
struct Util { unsigned gpu,memory; };
struct Memory { uint64_t total,free,used; };
static int metric_status(void) { return mode("unsupported")?3:mode("lost")?15:0; }
int nvmlDeviceGetUtilizationRates(void *dev, struct Util *v) { (void)dev; v->gpu=mode("bad-value")?101:38; v->memory=7; return metric_status(); }
int nvmlDeviceGetMemoryInfo(void *dev, struct Memory *v) { (void)dev; v->total=8589934592ULL; v->free=6442450944ULL; v->used=2147483648ULL; return metric_status(); }
int nvmlDeviceGetTemperature(void *dev, unsigned sensor, unsigned *v) { (void)dev;(void)sensor; *v=61; return metric_status(); }
int nvmlDeviceGetPowerUsage(void *dev, unsigned *v) { (void)dev; *v=118500; return metric_status(); }
#endif
