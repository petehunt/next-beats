import { agentTest } from 'cafo/node-test';

agentTest('native search submission', {
  app: {
    path: new URL('../', import.meta.url),
    port: 3000,
  },
  grader: {
    checklist: [
      'Submitting the Search tracks field with Enter navigates to /search?q=house and visibly shows matching search results.',
    ],
    maxInstances: 1,
    path: new URL('./', import.meta.url),
  },
  tester: {
    instructions: ({ appUrl }) =>
      `Open ${appUrl}/search. In the Search tracks field, enter "house" and submit it with Enter. Verify that the address remains on the search page with q=house in the query string and that matching results are visibly shown.`,
    path: new URL('./', import.meta.url),
    tokenSpendCapUsd: 0.5,
  },
});
