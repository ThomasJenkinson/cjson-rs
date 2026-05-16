/* Minimal cJSON.h subset — declares the symbols cjson-rs-ffi currently
 * exports. Mirrors upstream cJSON.h for the implemented surface; the full
 * 78-function surface will be added incrementally.
 */

#ifndef CJSON_H
#define CJSON_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Type bitfield constants (cJSON.h §"cJSON Types"). */
#define cJSON_Invalid (0)
#define cJSON_False   (1 << 0)
#define cJSON_True    (1 << 1)
#define cJSON_NULL    (1 << 2)
#define cJSON_Number  (1 << 3)
#define cJSON_String  (1 << 4)
#define cJSON_Array   (1 << 5)
#define cJSON_Object  (1 << 6)
#define cJSON_Raw     (1 << 7)

typedef int cJSON_bool;

typedef struct cJSON {
    struct cJSON *next;
    struct cJSON *prev;
    struct cJSON *child;
    int type;
    char *valuestring;
    int valueint;
    double valuedouble;
    char *string;
} cJSON;

/* Lifecycle */
const char *cJSON_Version(void);
cJSON *cJSON_Parse(const char *value);
cJSON *cJSON_ParseWithLength(const char *value, size_t buffer_length);
void   cJSON_Delete(cJSON *item);
char  *cJSON_Print(const cJSON *item);
char  *cJSON_PrintUnformatted(const cJSON *item);
void   cJSON_free(void *object);

/* Type predicates */
cJSON_bool cJSON_IsInvalid(const cJSON *item);
cJSON_bool cJSON_IsFalse(const cJSON *item);
cJSON_bool cJSON_IsTrue(const cJSON *item);
cJSON_bool cJSON_IsBool(const cJSON *item);
cJSON_bool cJSON_IsNull(const cJSON *item);
cJSON_bool cJSON_IsNumber(const cJSON *item);
cJSON_bool cJSON_IsString(const cJSON *item);
cJSON_bool cJSON_IsArray(const cJSON *item);
cJSON_bool cJSON_IsObject(const cJSON *item);
cJSON_bool cJSON_IsRaw(const cJSON *item);

/* Navigation and value getters */
int     cJSON_GetArraySize(const cJSON *array);
cJSON  *cJSON_GetArrayItem(const cJSON *array, int index);
cJSON  *cJSON_GetObjectItem(const cJSON *object, const char *string);
cJSON  *cJSON_GetObjectItemCaseSensitive(const cJSON *object, const char *string);
cJSON_bool cJSON_HasObjectItem(const cJSON *object, const char *string);
char   *cJSON_GetStringValue(const cJSON *item);
double  cJSON_GetNumberValue(const cJSON *item);

/* Constructors */
cJSON *cJSON_CreateNull(void);
cJSON *cJSON_CreateTrue(void);
cJSON *cJSON_CreateFalse(void);
cJSON *cJSON_CreateBool(cJSON_bool b);
cJSON *cJSON_CreateNumber(double num);
cJSON *cJSON_CreateString(const char *s);
cJSON *cJSON_CreateRaw(const char *raw);
cJSON *cJSON_CreateArray(void);
cJSON *cJSON_CreateObject(void);

/* Typed-array constructors */
cJSON *cJSON_CreateIntArray(const int *numbers, int count);
cJSON *cJSON_CreateFloatArray(const float *numbers, int count);
cJSON *cJSON_CreateDoubleArray(const double *numbers, int count);
cJSON *cJSON_CreateStringArray(const char *const *strings, int count);

/* Reference constructors */
cJSON *cJSON_CreateStringReference(const char *s);
cJSON *cJSON_CreateObjectReference(const cJSON *child);
cJSON *cJSON_CreateArrayReference(const cJSON *child);

/* Mutators */
cJSON_bool cJSON_AddItemToArray(cJSON *array, cJSON *item);
cJSON_bool cJSON_AddItemToObject(cJSON *object, const char *name, cJSON *item);
cJSON_bool cJSON_AddItemToObjectCS(cJSON *object, const char *name, cJSON *item);
cJSON_bool cJSON_AddItemReferenceToArray(cJSON *array, cJSON *item);
cJSON_bool cJSON_AddItemReferenceToObject(cJSON *object, const char *name, cJSON *item);

/* Detach / Delete / Insert / Replace */
cJSON *cJSON_DetachItemViaPointer(cJSON *parent, cJSON *item);
cJSON *cJSON_DetachItemFromArray(cJSON *array, int which);
void   cJSON_DeleteItemFromArray(cJSON *array, int which);
cJSON *cJSON_DetachItemFromObject(cJSON *object, const char *string);
cJSON *cJSON_DetachItemFromObjectCaseSensitive(cJSON *object, const char *string);
void   cJSON_DeleteItemFromObject(cJSON *object, const char *string);
void   cJSON_DeleteItemFromObjectCaseSensitive(cJSON *object, const char *string);
cJSON_bool cJSON_InsertItemInArray(cJSON *array, int which, cJSON *newitem);
cJSON_bool cJSON_ReplaceItemViaPointer(cJSON *parent, cJSON *item, cJSON *replacement);
cJSON_bool cJSON_ReplaceItemInArray(cJSON *array, int which, cJSON *newitem);
cJSON_bool cJSON_ReplaceItemInObject(cJSON *object, const char *string, cJSON *newitem);
cJSON_bool cJSON_ReplaceItemInObjectCaseSensitive(cJSON *object, const char *string, cJSON *newitem);

/* Misc */
cJSON *cJSON_Duplicate(const cJSON *item, cJSON_bool recurse);
void   cJSON_Minify(char *json);

/* Convenience helpers */
cJSON *cJSON_AddNullToObject(cJSON *object, const char *name);
cJSON *cJSON_AddTrueToObject(cJSON *object, const char *name);
cJSON *cJSON_AddFalseToObject(cJSON *object, const char *name);
cJSON *cJSON_AddBoolToObject(cJSON *object, const char *name, cJSON_bool boolean);
cJSON *cJSON_AddNumberToObject(cJSON *object, const char *name, double number);
cJSON *cJSON_AddStringToObject(cJSON *object, const char *name, const char *string);
cJSON *cJSON_AddRawToObject(cJSON *object, const char *name, const char *raw);
cJSON *cJSON_AddObjectToObject(cJSON *object, const char *name);
cJSON *cJSON_AddArrayToObject(cJSON *object, const char *name);

#ifdef __cplusplus
}
#endif

#endif /* CJSON_H */
