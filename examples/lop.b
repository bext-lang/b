main () {
     extrn printf;
     printf("LOP1: (%d)\n", 1 || 0);
     printf("LOP2: (%d)\n", 1 || 1);
     printf("LOP3: (%d)\n", 0 || 1);
     printf("LOP4: (%d)\n", 0 || 0);
     printf("LOP5: (%d)\n", 0 && 1);
     printf("LOP6: (%d)\n", 1 && 0);
     printf("LOP7: (%d)\n", 1 && 1);
     printf("LOP8: (%d)\n", 0 && 0);
}