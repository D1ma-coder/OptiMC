import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.core.BlockPos;
import net.minecraft.world.phys.shapes.CollisionContext;
public class ExportOutlineShapes {
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
  var shapes=new JsonArray(); var states=new JsonArray();
  var palette=new LinkedHashMap<String,Integer>(); int dynamic=0;
  for (var state:Block.BLOCK_STATE_REGISTRY) {
   var boxes=new JsonArray();
   for(var box:state.getShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO,CollisionContext.empty()).toAabbs()) {
    var a=new JsonArray(); for(double v:new double[]{box.minX,box.minY,box.minZ,box.maxX,box.maxY,box.maxZ}) a.add(v);
    boxes.add(a);
   }
   var key=boxes.toString(); var shape=palette.get(key);
   if(shape==null) {shape=palette.size();palette.put(key,shape);shapes.add(boxes);}
   var row=new JsonArray();row.add(shape);var block=state.getBlock();
   row.add(block.getFriction());row.add(block.getSpeedFactor());row.add(block.getJumpFactor());row.add(block.hasDynamicShape());
   if(block.hasDynamicShape())dynamic++;
   states.add(row);
  }
  var root=new JsonObject();root.addProperty("context","EmptyBlockGetter / BlockPos.ZERO / CollisionContext.empty");root.add("shapes",shapes);root.add("states",states);
  Files.writeString(Path.of(args[0]),new Gson().toJson(root));
  System.out.printf("Outline export: %d states, %d shapes, %d dynamic states%n",states.size(),shapes.size(),dynamic);
 }
}
