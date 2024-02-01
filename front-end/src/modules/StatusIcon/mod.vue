<template>
    <v-tooltip :location="location">
        <template v-slot:activator="{ props }">
            <v-icon v-if="isCorrect" color="green" icon="mdi-check-circle" v-bind="props"></v-icon>
            <v-icon v-if="!isCorrect" color="yellow" icon="mdi-alert-circle" v-bind="props"></v-icon>
        </template>
        <span v-if="isCorrect">{{ correctMessage }}</span>
        <span v-if="!isCorrect">{{ incorrectMessage }}</span>
    </v-tooltip>
</template>

<script>
export default {
    props: {
        isCorrect: Boolean,
        incorrectMessage: String,
        correctMessage: String,
        location: String,
    },
};
</script>
