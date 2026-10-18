#pragma once
#include <cstdint>
#define EC_PARAM_COUNT 216
extern "C" {
void* ec_daw_create(double rate);
void ec_daw_destroy(void*);
int ec_daw_set(void*,uint32_t,float);
int ec_daw_preset(void*,const float*,float);
int ec_daw_process(void*,float*,uint32_t,double,double,uint32_t);
uint32_t ec_daw_latency(const void*);
int ec_daw_step(const void*);
double ec_daw_tempo(const void*);
void ec_daw_defaults(float*);
int ec_daw_parse(uint32_t,const char*,const float*,float*);
void ec_daw_display(uint32_t,float,const float*,double,char*,uint32_t);
float ec_daw_duck(const void*);
int ec_library_read(char*,uint32_t);
int ec_library_write(const char*,uint32_t);
void ec_daw_name(uint32_t,char*,uint32_t);
float ec_daw_note_time(uint32_t);
float ec_daw_cc_value(uint32_t);
int ec_daw_program(void*,uint32_t,float*);
}
