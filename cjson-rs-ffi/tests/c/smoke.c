/* C-side smoke test for cjson-rs-ffi.
 *
 * Proves a plain C program can link against libcjson.{so,dylib} produced
 * by cargo and exercise the public API. Exits non-zero on any failure.
 */

#include "cjson.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(cond, msg)                                                      \
    do {                                                                       \
        if (!(cond)) {                                                         \
            fprintf(stderr, "FAIL: %s\n", msg);                                \
            return 1;                                                          \
        }                                                                      \
    } while (0)

static int test_version(void) {
    const char *v = cJSON_Version();
    CHECK(v != NULL, "cJSON_Version returned NULL");
    CHECK(strstr(v, "cjson-rs") != NULL, "version string did not contain cjson-rs");
    printf("  version: %s\n", v);
    return 0;
}

static int test_parse_print_round_trip(void) {
    const char *json = "{\"name\":\"alice\",\"age\":30,\"tags\":[\"admin\",\"user\"]}";
    cJSON *root = cJSON_Parse(json);
    CHECK(root != NULL, "parse returned NULL");
    CHECK(cJSON_IsObject(root), "root is not object");

    cJSON *name = cJSON_GetObjectItemCaseSensitive(root, "name");
    CHECK(name != NULL, "missing 'name'");
    CHECK(cJSON_IsString(name), "name is not string");
    CHECK(strcmp(cJSON_GetStringValue(name), "alice") == 0, "name value mismatch");

    cJSON *age = cJSON_GetObjectItemCaseSensitive(root, "age");
    CHECK(age != NULL, "missing 'age'");
    CHECK(cJSON_IsNumber(age), "age is not number");
    CHECK(cJSON_GetNumberValue(age) == 30.0, "age value mismatch");

    cJSON *tags = cJSON_GetObjectItemCaseSensitive(root, "tags");
    CHECK(tags != NULL, "missing 'tags'");
    CHECK(cJSON_IsArray(tags), "tags is not array");
    CHECK(cJSON_GetArraySize(tags) == 2, "tags size != 2");

    char *out = cJSON_PrintUnformatted(root);
    CHECK(out != NULL, "print returned NULL");
    printf("  parsed -> printed: %s\n", out);

    cJSON_free(out);
    cJSON_Delete(root);
    return 0;
}

static int test_null_safety(void) {
    /* All these must be safe to call with NULL inputs. */
    cJSON_Delete(NULL);
    cJSON_free(NULL);
    CHECK(cJSON_Parse(NULL) == NULL, "Parse(NULL) should be NULL");
    CHECK(cJSON_Print(NULL) == NULL, "Print(NULL) should be NULL");
    CHECK(cJSON_IsNumber(NULL) == 0, "IsNumber(NULL) should be false");
    CHECK(cJSON_GetArrayItem(NULL, 0) == NULL, "GetArrayItem(NULL, 0) should be NULL");
    CHECK(cJSON_GetObjectItem(NULL, "x") == NULL, "GetObjectItem(NULL, x) should be NULL");
    return 0;
}

static int test_field_walking_via_struct(void) {
    /* The cJSON struct fields are public ABI — many C consumers walk
     * children directly via item->child, item->next, item->string. Verify
     * that pattern works against our shim. */
    cJSON *root = cJSON_Parse("{\"a\":1,\"b\":2,\"c\":3}");
    CHECK(root != NULL, "parse failed");

    int count = 0;
    char expected_keys[3] = {'a', 'b', 'c'};
    cJSON *cur = root->child;
    while (cur != NULL) {
        CHECK(cur->string != NULL, "child missing key");
        CHECK(cur->string[0] == expected_keys[count],
              "key in unexpected position (insertion-order not preserved)");
        count++;
        cur = cur->next;
    }
    CHECK(count == 3, "expected 3 children");

    cJSON_Delete(root);
    return 0;
}

static int test_type_confusion_rejection(void) {
    /* Recent cJSON CVE was a type-confusion. GetNumberValue on a non-number
     * must return NaN (and not blindly read .valuedouble). */
    cJSON *s = cJSON_Parse("\"hello\"");
    CHECK(s != NULL, "parse failed");
    double n = cJSON_GetNumberValue(s);
    CHECK(n != n, "GetNumberValue on string should return NaN");
    cJSON_Delete(s);
    return 0;
}

static int test_deep_nesting_rejected(void) {
    /* Adversarial: 2000 levels of nesting. Default limit is 1000. */
    size_t depth = 2000;
    char *buf = malloc(depth * 2 + 2);
    CHECK(buf != NULL, "malloc failed");
    for (size_t i = 0; i < depth; i++) buf[i] = '[';
    buf[depth] = '1';
    for (size_t i = 0; i < depth; i++) buf[depth + 1 + i] = ']';
    buf[depth * 2 + 1] = '\0';

    cJSON *root = cJSON_Parse(buf);
    CHECK(root == NULL, "deeply nested input should be rejected");
    free(buf);
    return 0;
}

static int test_build_with_helpers(void) {
    /* Construct {"name":"alice","age":30,"tags":["admin","user"]} from scratch
     * using the convenience helpers, then verify the printed output. */
    cJSON *root = cJSON_CreateObject();
    CHECK(root != NULL, "CreateObject NULL");
    CHECK(cJSON_AddStringToObject(root, "name", "alice") != NULL, "AddString failed");
    CHECK(cJSON_AddNumberToObject(root, "age", 30.0) != NULL, "AddNumber failed");

    cJSON *tags = cJSON_AddArrayToObject(root, "tags");
    CHECK(tags != NULL, "AddArrayToObject failed");
    CHECK(cJSON_AddItemToArray(tags, cJSON_CreateString("admin")), "AddItem admin failed");
    CHECK(cJSON_AddItemToArray(tags, cJSON_CreateString("user")), "AddItem user failed");

    char *out = cJSON_PrintUnformatted(root);
    CHECK(out != NULL, "Print returned NULL");
    CHECK(strcmp(out, "{\"name\":\"alice\",\"age\":30,\"tags\":[\"admin\",\"user\"]}") == 0,
          "built output did not match expected");
    cJSON_free(out);
    cJSON_Delete(root);
    return 0;
}

static int test_typed_array_constructors(void) {
    int ints[] = {1, 2, 3, 4, 5};
    cJSON *arr = cJSON_CreateIntArray(ints, 5);
    CHECK(arr != NULL, "CreateIntArray NULL");
    char *out = cJSON_PrintUnformatted(arr);
    CHECK(strcmp(out, "[1,2,3,4,5]") == 0, "int array output mismatch");
    cJSON_free(out);
    cJSON_Delete(arr);
    return 0;
}

static int test_mutators(void) {
    /* Build a small tree, mutate via Detach/Insert/Replace, verify
     * via the printed output. */
    cJSON *arr = cJSON_Parse("[10,20,30]");
    CHECK(arr != NULL, "parse failed");

    /* Detach middle, free it, verify size */
    cJSON *mid = cJSON_DetachItemFromArray(arr, 1);
    CHECK(mid != NULL, "detach NULL");
    cJSON_Delete(mid);
    CHECK(cJSON_GetArraySize(arr) == 2, "size after detach != 2");

    /* Insert 99 at index 0 */
    cJSON *n = cJSON_CreateNumber(99.0);
    CHECK(cJSON_InsertItemInArray(arr, 0, n), "insert failed");

    /* Replace at index 1 with string */
    cJSON *rep = cJSON_CreateString("replaced");
    CHECK(cJSON_ReplaceItemInArray(arr, 1, rep), "replace failed");

    char *out = cJSON_PrintUnformatted(arr);
    CHECK(strcmp(out, "[99,\"replaced\",30]") == 0, "unexpected output after mutations");
    cJSON_free(out);

    /* Duplicate then verify independence */
    cJSON *copy = cJSON_Duplicate(arr, 1);
    cJSON_DeleteItemFromArray(copy, 0);
    CHECK(cJSON_GetArraySize(arr) == 3, "src array was modified by copy mutation");
    cJSON_Delete(copy);
    cJSON_Delete(arr);
    return 0;
}

static int test_minify(void) {
    char buf[] = "  {\n  \"a\" : 1 ,\n  \"b\" : [1, 2]\n}  ";
    cJSON_Minify(buf);
    CHECK(strcmp(buf, "{\"a\":1,\"b\":[1,2]}") == 0, "minify output mismatch");
    return 0;
}

static int test_string_reference_no_double_free(void) {
    /* Caller-owned static; cJSON_CreateStringReference must not try to free it. */
    static const char borrowed[] = "borrowed";
    cJSON *n = cJSON_CreateStringReference(borrowed);
    CHECK(n != NULL, "CreateStringReference NULL");
    CHECK(strcmp(cJSON_GetStringValue(n), "borrowed") == 0, "reference value wrong");
    cJSON_Delete(n);
    /* If Delete had freed `borrowed` we'd have undefined behaviour here. */
    CHECK(borrowed[0] == 'b', "static string corrupted by Delete");
    return 0;
}

int main(void) {
    printf("cjson-rs-ffi C smoke test\n");
    int rc = 0;
    rc |= test_version();                       printf("  test_version OK\n");
    rc |= test_parse_print_round_trip();        printf("  test_parse_print_round_trip OK\n");
    rc |= test_null_safety();                   printf("  test_null_safety OK\n");
    rc |= test_field_walking_via_struct();      printf("  test_field_walking_via_struct OK\n");
    rc |= test_type_confusion_rejection();      printf("  test_type_confusion_rejection OK\n");
    rc |= test_deep_nesting_rejected();         printf("  test_deep_nesting_rejected OK\n");
    rc |= test_build_with_helpers();            printf("  test_build_with_helpers OK\n");
    rc |= test_typed_array_constructors();      printf("  test_typed_array_constructors OK\n");
    rc |= test_mutators();                       printf("  test_mutators OK\n");
    rc |= test_minify();                         printf("  test_minify OK\n");
    rc |= test_string_reference_no_double_free(); printf("  test_string_reference_no_double_free OK\n");
    if (rc == 0) printf("ALL OK\n");
    return rc;
}
