#ifndef KB_MATH_H
#define KB_MATH_H

#define INFINITY (__builtin_inff())
#define NAN (__builtin_nanf(""))
#define isnan __builtin_isnan
#define isinf __builtin_isinf
#define signbit __builtin_signbit
#define copysign __builtin_copysign
#define copysignf __builtin_copysignf

float fabsf(float);
float sqrtf(float);
float floorf(float);
float ceilf(float);
float truncf(float);
float nearbyintf(float);
double fabs(double);
double sqrt(double);
double floor(double);
double ceil(double);
double trunc(double);
double nearbyint(double);

#endif
