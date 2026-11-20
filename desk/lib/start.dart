/// Womit das Fenster aufgehen soll, wenn es nicht die bloße Übersicht ist.
///
/// - `--edit NAME`: das Profil gleich zum Bearbeiten öffnen
/// - `--new`: ein neues Profil anlegen
/// - `--secrets`: die Geheimnisse öffnen
///
/// Gedacht für Verknüpfungen, und damit sich jede Ansicht des echten Fensters
/// ansehen lässt, ohne zu klicken.
class StartRequest {
  const StartRequest.edit(String this.profile) : newProfile = false, secrets = false;
  const StartRequest.newProfile() : profile = null, newProfile = true, secrets = false;
  const StartRequest.secrets() : profile = null, newProfile = false, secrets = true;

  final String? profile;
  final bool newProfile;
  final bool secrets;

  static StartRequest? fromArgs(List<String> args) {
    for (final (index, arg) in args.indexed) {
      if (arg == '--new') {
        return const StartRequest.newProfile();
      }
      if (arg == '--secrets') {
        return const StartRequest.secrets();
      }
      if (arg == '--edit' && index + 1 < args.length) {
        return StartRequest.edit(args[index + 1]);
      }
    }
    return null;
  }
}
