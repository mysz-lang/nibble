#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void print_int(int a) {
    printf("%d\n", a);
}

void print_str(const char* s) {
    if (s != NULL) {
        printf("%s\n", s);
    }
}

/* * ABI Concat Driver Hook
 * Dynamically binds string expressions mapping back from the Mysz '+' token rules
 */
char* str_concat(const char* str1, const char* str2) {
    if (str1 == NULL) str1 = "";
    if (str2 == NULL) str2 = "";

    size_t length = strlen(str1) + strlen(str2) + 1;
    char* target_buffer = malloc(length);
    
    if (target_buffer == NULL) {
        fprintf(stderr, "[nibble-runtime] FATAL: Heap allocation out of memory inside str_concat loop.\n");
        exit(1);
    }
    
    strcpy(target_buffer, str1);
    strcat(target_buffer, str2);
    return target_buffer;
}