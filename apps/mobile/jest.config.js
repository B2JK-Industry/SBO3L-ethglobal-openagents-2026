module.exports = {
  preset: "jest-expo",
  testMatch: ["**/__tests__/**/*.test.ts?(x)"],
  moduleNameMapper: { "^~/(.*)$": "<rootDir>/src/$1" },
  setupFilesAfterEach: [],
  transformIgnorePatterns: [
    "node_modules/(?!(expo|expo-.*|@expo/.*|expo-modules-core|react-native|@react-native|@react-navigation)/)",
  ],
};
