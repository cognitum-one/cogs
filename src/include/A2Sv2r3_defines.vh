/*==============================================================================================================================

   Copyright © 2022 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : "Common"
   Description          : Common Configuration file

   Version              : 1.0

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

`ifndef  A2Sv2r3_DEFINES
   `define  A2Sv2r3_DEFINES

// CONFIGURATION

   `define  A2S_ENABLE_INTERRUPTS
// `define  A2S_ENABLE_SPILL_FILL                        // Needs Interrupts

   `ifdef   A2S_ENABLE_INTERRUPTS
      `ifndef  A2S_ENABLE_INTERRUPTS
         `define  A2S_ENABLE_INTERRUPTS
      `endif //A2S_ENABLE_INTERRUPTS
   `endif //A2S_ENABLE_INTERRUPTS

   `define  A2S_ENABLE_SHIFTS
   `define  A2S_ENABLE_FLOATING_POINT
   `define  A2S_ENABLE_SIMD
   `define  A2S_ENABLE_INTEGER_MULTIPLY
   `define  A2S_ENABLE_INTEGER_DIVIDE
   `define  A2S_ENABLE_CONDITION_CODES
   `define  A2S_ENABLE_POPULATION_COUNT
   `define  A2S_ENABLE_SWAPBYTES
   `define  A2S_NUM_CONDITION_CODES    10
   `define  A2S_NUM_EXTN_INTERRUPTS     4
   `define  A2S_STATUS_REG_WIDTH        4

// BASE PARAMETERS

   `define  A2S_MACHINE_WIDTH          32
   `define  A2S_INSTRUCTION_WIDTH      32
   `define  A2S_ORDER_WIDTH             6
   `define  A2S_IMMEDIATE_WIDTH        16
   `define  A2S_FUNCTION_WIDTH         16
   `define  A2S_RTRN_STACK_DEPTH       32                // Limit is 1024
   `define  A2S_DATA_STACK_DEPTH       32                // Limit is 1024
   `define  A2S_IO_ADDRESS_WIDTH       13

// I/O & MEMORY FIELD SIZES                              // Prototypes for collation and decollation

   `define  DMOSI_WIDTH    1+1+1+32+32                   // =  67 {eDM, rDM, wDM, iDM[31:0], aDM[31:0]}
   `define  DMISO_WIDTH    1+1+32                        // =  34 {DMg, DMk,      DMo[31:0]}

   `define  IMOSI_WIDTH    1+2+32+13                     // =  48 {eIO, cIO[1:0],iIO[31:0], aCM[12:0]}
   `define  IMISO_WIDTH    1+  32                        // =  33 {     IOk, IOo[31:0]}

   `define  WMOSI_WIDTH    1+1+128+128+32                // = 290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
   `define  WMISO_WIDTH    1+1+128                       // = 130 {WMg, WMk, WMo[127:0]}

   `define  FMOSI_WIDTH    1+1+1+8+32+32+32              // = 107 {eFP,  imul, istall, cFP[7:0],  F3[31:0], F2[31:0], F1[31:0]}
   `define  FMISO_WIDTH    1+1+1+32+64                   // =  99 {FPit, FPex, FPir,    Fo[31:0], Mo[63:0] }

   `define  PMISO_WIDTH    1+1+1+32                      // =  35 { SIMDir,SIMDor,SIMDex,                 SIMDo[31:0]}
   `define  PMOSI_WIDTH    1+1+9+4+32                    // =  47 {eSIMD, tSIMD, cSIMD[8:0], mSIMD[3:0], iSIMD [31:0]}

// A2S System Registers in IO space

   `define  index_A2S_ST      13'h1fff
   `define  index_A2S_RS      13'h1ffe
   `define  index_A2S_DS      13'h1ffd
   `define  index_A2S_IN      13'h1ffc
   `define  index_A2S_IE      13'h1ffb
   `define  index_A2S_EP      13'h1ffa

   `define  index_A2S_SFPR    13'h1ff3
   `define  index_A2S_SFPD    13'h1ff2
   `define  index_A2S_SFDR    13'h1ff1
   `define  index_A2S_SFDD    13'h1ff0

// Floating Point & Integer Multiply & Divide
// If    FPU use integer multiply from FPU and optionally add integer divider
// If no FPU use MULDIV use integer multilpy and get divide 'for free'
// These defines set above at top of this file

   `ifdef   A2S_ENABLE_FLOATING_POINT

      `define  USE_FPU                          // Single Precision Floating Point Unit (no Divide et al)
      `define  USE_P3P1                         // Set for FMAC operations

      `ifdef   A2S_ENABLE_INTEGER_MULTIPLY
         `define  USE_PIM                       // Parallel Integer Multiplier built into FPU
      `endif //A2S_ENABLE_INTEGER_MULTIPLY

      `ifdef   A2S_ENABLE_INTEGER_DIVIDE        // Serial Integer Multiplier
         `define  USE_DIV
         `define  USE_P2P2
      `else
         `define  NO_SERIAL_INTEGER_MULDIV
      `endif //A2S_ENABLE_INTEGER_DIVIDE

   `else  //A2S_ENABLE_FLOATING_POINT

      `ifdef   A2S_ENABLE_INTEGER_MULTIPLY
         `define  USE_MULDIV                    // Combined Interger Multiplier/Divider

         `ifndef  USE_P2P2
            `define  USE_P2P2
         `endif //USE_P2P2

      `else

         `ifdef   A2S_ENABLE_INTEGER_DIVIDE
            `define  USE_DIV                    // Serial Integer Divider

            `ifndef  USE_P2P2
               `define  USE_P2P2
            `endif //USE_P2P2

         `endif //A2S_ENABLE_INTEGER_DIVIDE

      `endif // A2S_ENABLE_INTEGER_MULTIPLY

      `define  NO_INTEGER_MULDIV

   `endif //A2S_ENABLE_FLOATING_POINT

`endif //A2Sv2r3_DEFINES

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
