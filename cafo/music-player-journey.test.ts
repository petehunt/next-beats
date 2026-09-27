import { agentTest, baselineQualityChecks } from 'cafo/node-test';

agentTest('authenticated music player journey', [baselineQualityChecks()], {
  app: {
    path: new URL('../', import.meta.url),
    port: 3000,
  },
  grader: {
    checklist: [
      'A signed-out visitor can sign in with aurora@example.com and reaches the music home page.',
      'The signed-in user can browse the home, library, search genre browse, and playlist areas and sees music content in each visited area.',
      'Opening a track and playing it visibly updates the persistent now-playing controls with that track.',
      'Favoriting a track makes it appear on Liked Tracks.',
      'Creating a playlist from a track and adding that track succeeds, and the new playlist can be opened with the track present.',
      'At a narrow mobile viewport, the fixed bottom navigation remains usable to reach Search and Library.',
    ],
    maxInstances: 2,
    path: new URL('./', import.meta.url),
  },
  tester: {
    instructions: ({ appUrl }) =>
      `Open ${appUrl}/login and sign in with aurora@example.com. Exercise the representative music-player flow: inspect the home content; visit Library and Search, browse a genre, and return to the music content; open a track, start playback, and visibly confirm it appears in the persistent now-playing bar; favorite that track and confirm it appears on Liked Tracks. From the track's Add to playlist control, create a uniquely named playlist, confirm the track is added, and open that playlist to verify it contains the track. Finally resize to a narrow mobile viewport and use the bottom navigation to visit Search and Library, confirming both render usable content.`,
    path: new URL('./', import.meta.url),
    tokenSpendCapUsd: 1.5,
  },
});
