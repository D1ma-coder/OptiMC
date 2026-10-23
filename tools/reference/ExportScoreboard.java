// Development-only CPU reference: original 26.2 codecs; no game/server or GPU.
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.chat.*;
import net.minecraft.network.chat.numbers.*;
import net.minecraft.network.protocol.game.*;
import net.minecraft.world.scores.*;
import net.minecraft.world.scores.criteria.ObjectiveCriteria;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;

public class ExportScoreboard {
 static JsonArray packets = new JsonArray();
 static <T> void add(String label, int id, StreamCodec<? super RegistryFriendlyByteBuf,T> codec, T packet) {
  var buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(), RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  codec.encode(buffer, packet); byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes); buffer.release();
  var row = new JsonObject(); row.addProperty("label",label); row.addProperty("id",id); row.addProperty("body",Base64.getEncoder().encodeToString(bytes)); packets.add(row);
 }
 static void score(String owner, int value, Component display, NumberFormat format) {
  add("score_"+owner,0x6e,ClientboundSetScorePacket.STREAM_CODEC,new ClientboundSetScorePacket(owner,"main",value,Optional.ofNullable(display),Optional.ofNullable(format)));
 }
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
  var board = new Scoreboard();
  var objective = board.addObjective("main",ObjectiveCriteria.DUMMY,Component.literal("Points"),ObjectiveCriteria.RenderType.INTEGER,false,null);
  add("objective",0x6a,ClientboundSetObjectivePacket.STREAM_CODEC,new ClientboundSetObjectivePacket(objective,0));
  add("sidebar",0x62,ClientboundSetDisplayObjectivePacket.STREAM_CODEC,new ClientboundSetDisplayObjectivePacket(DisplaySlot.SIDEBAR,objective));
  score("Alpha",9,null,null);
  score("beta",9,Component.literal("Beta!").withStyle(Style.EMPTY.withColor(0x55ffff)),new StyledFormat(Style.EMPTY.withColor(0xffaa00).withBold(true)));
  score("blank",7,null,BlankFormat.INSTANCE);
  score("fixed",6,null,new FixedFormat(Component.literal("WIN").withStyle(Style.EMPTY.withColor(0x55ff55))));
  score("#hidden",999,null,null);
  var team = board.addPlayerTeam("red"); team.setDisplayName(Component.literal("Red team"));
  team.setPlayerPrefix(Component.literal("[R] ")); team.setPlayerSuffix(Component.literal("!"));
  team.setColor(Optional.of(TeamColor.RED)); team.setNameTagVisibility(Team.Visibility.HIDE_FOR_OTHER_TEAMS);
  team.setCollisionRule(Team.CollisionRule.PUSH_OWN_TEAM); team.setAllowFriendlyFire(false); team.setSeeFriendlyInvisibles(true);
  board.addPlayerToTeam("Alpha",team); board.addPlayerToTeam("Fixture",team);
  add("team_add",0x6d,ClientboundSetPlayerTeamPacket.STREAM_CODEC,ClientboundSetPlayerTeamPacket.createAddOrModifyPacket(team,true));
  add("colored_sidebar",0x62,ClientboundSetDisplayObjectivePacket.STREAM_CODEC,new ClientboundSetDisplayObjectivePacket(DisplaySlot.TEAM_RED,objective));
  team.setPlayerPrefix(Component.literal("[NEW] ")); team.setColor(Optional.empty());
  add("team_update",0x6d,ClientboundSetPlayerTeamPacket.STREAM_CODEC,ClientboundSetPlayerTeamPacket.createAddOrModifyPacket(team,false));
  add("team_join",0x6d,ClientboundSetPlayerTeamPacket.STREAM_CODEC,ClientboundSetPlayerTeamPacket.createPlayerPacket(team,"beta",ClientboundSetPlayerTeamPacket.Action.ADD));
  add("team_leave",0x6d,ClientboundSetPlayerTeamPacket.STREAM_CODEC,ClientboundSetPlayerTeamPacket.createPlayerPacket(team,"Alpha",ClientboundSetPlayerTeamPacket.Action.REMOVE));
  objective.setDisplayName(Component.literal("Updated")); objective.setRenderType(ObjectiveCriteria.RenderType.HEARTS); objective.setNumberFormat(BlankFormat.INSTANCE);
  add("objective_update",0x6a,ClientboundSetObjectivePacket.STREAM_CODEC,new ClientboundSetObjectivePacket(objective,2));
  add("reset_one",0x4f,ClientboundResetScorePacket.STREAM_CODEC,new ClientboundResetScorePacket("beta","main"));
  add("reset_all",0x4f,ClientboundResetScorePacket.STREAM_CODEC,new ClientboundResetScorePacket("Alpha",null));
  add("team_remove",0x6d,ClientboundSetPlayerTeamPacket.STREAM_CODEC,ClientboundSetPlayerTeamPacket.createRemovePacket(team));
  add("objective_remove",0x6a,ClientboundSetObjectivePacket.STREAM_CODEC,new ClientboundSetObjectivePacket(objective,1));
  var result = new JsonObject(); result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754"); result.add("packets",packets);
  Files.writeString(Path.of(args[0],"scoreboard-reference.json"),new Gson().toJson(result));
  var folds = new JsonArray();
  for(int cp=0; cp<=Character.MAX_CODE_POINT; cp++) {
   int folded=Character.toLowerCase(Character.toUpperCase(cp));
   if(cp!=folded) { var pair=new JsonArray();pair.add(cp);pair.add(folded);folds.add(pair); }
  }
  var caseData = new JsonObject(); caseData.addProperty("java_version",System.getProperty("java.version"));caseData.add("folds",folds);
  var cases = new JsonArray();
  for(String a : List.of("Alpha","alpha","beta","İ","ı","ß","SS","Σ","ς","σ","\uE000","𐐀","𐐨"))
   for(String b : List.of("Alpha","alpha","beta","İ","ı","ß","SS","Σ","ς","σ","\uE000","𐐀","𐐨")) {
    var pair = new JsonArray();pair.add(a);pair.add(b);pair.add(Integer.signum(String.CASE_INSENSITIVE_ORDER.compare(a,b)));cases.add(pair);
   }
  caseData.add("comparisons",cases);Files.writeString(Path.of(args[0],"java-case-fold.json"),new Gson().toJson(caseData));
  System.out.println("Exported "+packets.size()+" original scoreboard packet bodies and "+folds.size()+" Java case folds");
 }
}
