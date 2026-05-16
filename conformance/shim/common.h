/* Shim common.h — replaces upstream tests/common.h so that the upstream
 * test files link against libcjson.dylib (produced by cjson-rs-ffi)
 * instead of compiling upstream's cJSON.c directly.
 *
 * The only deviations from upstream's common.h are:
 *  - Includes the upstream cJSON.h (declarations only) instead of
 *    cJSON.c (which would pull in upstream's whole implementation).
 *  - reset() uses stdlib free() instead of global_hooks.deallocate(),
 *    because global_hooks is an upstream-internal symbol.
 *
 * This file is pre-included via the compiler's -include flag, so the
 * test file's own `#include "common.h"` becomes a no-op (header guard).
 */

#ifndef CJSON_TESTS_COMMON_H
#define CJSON_TESTS_COMMON_H

#include "cJSON.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>
#include <math.h>
#include <float.h>

/* compare_double is defined as static in upstream cJSON.c, but readme_examples.c
 * uses it. Re-provide here so the test can link against our libcjson.dylib. */
static inline cJSON_bool compare_double(double a, double b) {
    double maxVal = fabs(a) > fabs(b) ? fabs(a) : fabs(b);
    return (fabs(a - b) <= maxVal * DBL_EPSILON);
}

static inline void reset(cJSON *item) {
    if ((item != NULL) && (item->child != NULL)) {
        cJSON_Delete(item->child);
    }
    if ((item->valuestring != NULL) && !(item->type & cJSON_IsReference)) {
        free(item->valuestring);
    }
    if ((item->string != NULL) && !(item->type & cJSON_StringIsConst)) {
        free(item->string);
    }
    memset(item, 0, sizeof(cJSON));
}

static inline char* read_file(const char *filename) {
    FILE *file = NULL;
    long length = 0;
    char *content = NULL;
    size_t read_chars = 0;

    file = fopen(filename, "rb");
    if (file == NULL) goto cleanup;

    if (fseek(file, 0, SEEK_END) != 0) goto cleanup;
    length = ftell(file);
    if (length < 0) goto cleanup;
    if (fseek(file, 0, SEEK_SET) != 0) goto cleanup;

    content = (char*)malloc((size_t)length + 1);
    if (content == NULL) goto cleanup;

    read_chars = fread(content, sizeof(char), (size_t)length, file);
    if ((long)read_chars != length) {
        free(content);
        content = NULL;
        goto cleanup;
    }
    content[read_chars] = '\0';

cleanup:
    if (file != NULL) fclose(file);
    return content;
}

#define assert_has_type(item, item_type) TEST_ASSERT_BITS_MESSAGE(0xFF, item_type, item->type, "Item doesn't have expected type.")
#define assert_has_no_reference(item) TEST_ASSERT_BITS_MESSAGE(cJSON_IsReference, 0, item->type, "Item should not have a string as reference.")
#define assert_has_no_const_string(item) TEST_ASSERT_BITS_MESSAGE(cJSON_StringIsConst, 0, item->type, "Item should not have a const string.")
#define assert_has_valuestring(item) TEST_ASSERT_NOT_NULL_MESSAGE(item->valuestring, "Valuestring is NULL.")
#define assert_has_no_valuestring(item) TEST_ASSERT_NULL_MESSAGE(item->valuestring, "Valuestring is not NULL.")
#define assert_has_string(item) TEST_ASSERT_NOT_NULL_MESSAGE(item->string, "String is NULL")
#define assert_has_no_string(item) TEST_ASSERT_NULL_MESSAGE(item->string, "String is not NULL.")
#define assert_not_in_list(item) \
    TEST_ASSERT_NULL_MESSAGE(item->next, "Linked list next pointer is not NULL.");\
    TEST_ASSERT_NULL_MESSAGE(item->prev, "Linked list previous pointer is not NULL.")
#define assert_has_child(item) TEST_ASSERT_NOT_NULL_MESSAGE(item->child, "Item doesn't have a child.")
#define assert_has_no_child(item) TEST_ASSERT_NULL_MESSAGE(item->child, "Item has a child.")
#define assert_is_invalid(item) \
    assert_has_type(item, cJSON_Invalid);\
    assert_not_in_list(item);\
    assert_has_no_child(item);\
    assert_has_no_string(item);\
    assert_has_no_valuestring(item)

#endif
