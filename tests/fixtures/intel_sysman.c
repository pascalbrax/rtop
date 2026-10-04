/* Test-only Level Zero Sysman ABI fixture. All handles are synthetic. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int mode(const char *s) {const char *m=getenv("RTOP_FAKE_INTEL");return m&&!strcmp(m,s);}
static uint32_t metric_status(void){return mode("unsupported")?0x78000003:mode("permission")?0x70010000:mode("lost")?0x70000001:0;}
struct Pci {uint32_t stype;void *next;uint32_t address[4];int32_t gen,width;int64_t bandwidth;uint8_t counters[3];};
struct Engine {uint32_t stype;void *next;uint32_t kind;uint8_t subdevice;uint32_t id;};
struct Counters {uint64_t active,time;};
struct Memory {uint32_t stype;void *next;uint32_t kind;uint8_t subdevice;uint32_t id,location;uint64_t physical;int32_t bus_width,channels;};
struct MemoryState {uint32_t stype;void *next;uint32_t health;uint64_t free,size;};
struct Temperature {uint32_t stype;void *next;uint32_t kind;uint8_t subdevice;uint32_t id;double maximum;uint8_t critical,threshold1,threshold2;};
#ifndef OLD_LOADER
uint32_t zesInit(uint32_t flags){(void)flags;if(mode("slow"))sleep(10);return mode("init-error")?0x78000001:0;}
#endif
uint32_t zesDriverGet(uint32_t *count,void **out){if(!out){*count=mode("excessive")?129:mode("zero")?0:1;}else{out[0]=mode("null")?NULL:(void*)1;if(mode("growing"))*count=2;}return mode("count-error")?0x70000001:0;}
uint32_t zesDeviceGet(void *driver,uint32_t *count,void **out){(void)driver;if(!out)*count=mode("no-device")?0:1;else out[0]=(void*)2;return 0;}
uint32_t zesDevicePciGetProperties(void *device,struct Pci *p){(void)device;if(p->stype!=2||p->next)return 0x78000004;p->address[0]=0;p->address[1]=mode("wrong-pci")?4:3;p->address[2]=0;p->address[3]=0;return mode("pci-error")?0x70010000:0;}
#ifndef REQUIRED_ONLY
static uint32_t component(uint32_t *count,void **out,uintptr_t handle){if(!out)*count=mode("no-components")?0:1;else out[0]=(void*)handle;return metric_status();}
uint32_t zesDeviceEnumEngineGroups(void *dev,uint32_t *count,void **out){(void)dev;return component(count,out,3);}
uint32_t zesEngineGetProperties(void *engine,struct Engine *p){(void)engine;if(p->stype!=5||p->next)return 0x78000004;p->kind=mode("single-engine")?5:0;p->subdevice=0;return 0;}
uint32_t zesEngineGetActivity(void *engine,struct Counters *c){(void)engine;static unsigned calls;calls++;c->time=(uint64_t)calls*1000000;c->active=(uint64_t)calls*250000;if(mode("no-progress"))c->time=1000000;if(mode("reset")&&calls>1)c->active=1;if(mode("bad-activity")&&calls>1)c->active=5000000;return metric_status();}
uint32_t zesDeviceEnumMemoryModules(void *dev,uint32_t *count,void **out){(void)dev;if(mode("tiles")){if(!out)*count=3;else{out[0]=(void*)4;out[1]=(void*)6;out[2]=(void*)7;}return 0;}return component(count,out,4);}
uint32_t zesMemoryGetProperties(void *memory,struct Memory *p){if(p->stype!=0xb||p->next)return 0x78000004;p->location=mode("system-memory")?0:1;p->subdevice=memory!=(void*)4;p->id=0;p->physical=mode("old-memory-size")?0:8589934592ULL;return 0;}
uint32_t zesMemoryGetState(void *memory,struct MemoryState *p){(void)memory;if(p->stype!=0x1e||p->next)return 0x78000004;p->size=8589934592ULL;p->free=mode("bad-memory")?9000000000ULL:6442450944ULL;return metric_status();}
uint32_t zesDeviceEnumTemperatureSensors(void *dev,uint32_t *count,void **out){(void)dev;return component(count,out,5);}
uint32_t zesTemperatureGetProperties(void *temp,struct Temperature *p){(void)temp;if(p->stype!=0x14||p->next)return 0x78000004;p->kind=1;p->subdevice=0;p->maximum=125;p->critical=1;return 0;}
uint32_t zesTemperatureGetState(void *temp,double *v){(void)temp;*v=mode("bad-temp")?2000:58.5;return metric_status();}
#endif
