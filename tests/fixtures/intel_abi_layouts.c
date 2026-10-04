/* Optional ABI check against the unmodified official Level Zero headers. */
#include <zes_api.h>
#include <stddef.h>
#include <stdio.h>
_Static_assert(sizeof(ze_bool_t)==1,"bool ABI");
_Static_assert(sizeof(zes_pci_properties_t)==56,"PCI ABI");
_Static_assert(offsetof(zes_pci_properties_t,address)==16,"PCI address ABI");
_Static_assert(sizeof(zes_engine_properties_t)==32,"engine ABI");
_Static_assert(sizeof(zes_engine_stats_t)==16,"counter ABI");
_Static_assert(sizeof(zes_mem_properties_t)==48,"memory ABI");
_Static_assert(offsetof(zes_mem_properties_t,physicalSize)==32,"physical memory ABI");
_Static_assert(sizeof(zes_mem_state_t)==40,"memory state ABI");
_Static_assert(offsetof(zes_mem_state_t,free)==24,"memory free ABI");
_Static_assert(sizeof(zes_temp_properties_t)==48,"temperature ABI");
_Static_assert(offsetof(zes_temp_properties_t,maxTemperature)==32,"temperature maximum ABI");
_Static_assert(ZES_STRUCTURE_TYPE_MEM_STATE==0x1e,"memory stype ABI");
_Static_assert(ZES_MEM_LOC_DEVICE==1,"device-local memory ABI");
_Static_assert(ZES_ENGINE_GROUP_ALL==0,"whole-device engine ABI");
int main(void){puts("PASS: official Sysman C ABI sizes, offsets and enum values (64-bit Linux)");}
